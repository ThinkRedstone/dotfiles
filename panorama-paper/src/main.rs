mod layout;
mod panorama;
mod renderer;

use clap::Parser;
use glam::{DQuat, DVec3};
use layout::{Layout, Rectangle};
use panorama::Panorama;
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use renderer::{Camera, Renderer, Target};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData},
    delegate_registry,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{
            EventLoop,
            timer::{TimeoutAction, Timer},
        },
        calloop_wayland_source::WaylandSource,
        protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{
            Shape, WpCursorShapeDeviceV1,
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        pointer::{
            PointerEvent, PointerEventKind, PointerHandler, cursor_shape::CursorShapeManager,
        },
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
};
use std::{
    error::Error,
    path::PathBuf,
    ptr::NonNull,
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Proxy, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_surface},
};

/// Animated Minecraft-style title-screen panorama as a Wayland wallpaper.
#[derive(Parser)]
struct Arguments {
    /// Resource-pack zip or directory containing panorama_0.png … panorama_5.png
    panorama: PathBuf,
    /// Rotation speed in degrees per second; with --follow-cursor, the speed with the cursor at the
    /// screen edge
    #[arg(long, default_value_t = 2.0)]
    speed: f64,
    /// Steer with the cursor: its offset from the center of the screen (or of the whole layout
    /// with --span) sets the rotation velocity, which persists while the cursor is over a window
    #[arg(long)]
    follow_cursor: bool,
    /// Render one continuous panorama across all monitors; the default unless --output is given
    #[arg(long, overrides_with = "no_span")]
    span: bool,
    /// Render a separate, centered view on each monitor
    #[arg(long, overrides_with = "span")]
    no_span: bool,
    /// Vertical field of view in degrees; with --span, across the tallest landscape monitor
    #[arg(long, default_value_t = 85.0)]
    fov: f32,
    /// How far the camera looks down, in degrees
    #[arg(long, default_value_t = 10.0, allow_negative_numbers = true)]
    pitch: f32,
    /// Frames per second. Every frame makes the compositor redraw the whole output, so this is
    /// what the wallpaper's power draw scales with
    #[arg(long, default_value_t = 30.0)]
    fps: f64,
    /// Only draw on these outputs (e.g. DP-6); defaults to all
    #[arg(long = "output")]
    outputs: Vec<String>,
    /// Render one frame to this PNG and exit, instead of running as a wallpaper
    #[arg(long, value_name = "FILE")]
    screenshot: Option<PathBuf>,
    /// Screenshot size in pixels
    #[arg(long, value_name = "WIDTHxHEIGHT", default_value = "2560x1440", value_parser = parse_size)]
    screenshot_size: (u32, u32),
    /// Seconds into the rotation at which to take the screenshot
    #[arg(long, value_name = "SECONDS", default_value_t = 0.0)]
    screenshot_time: f64,
}

fn parse_size(size: &str) -> Result<(u32, u32), String> {
    let (width, height) = size.split_once('x').ok_or("expected WIDTHxHEIGHT")?;
    let parse = |value: &str| value.parse::<u32>().map_err(|error| error.to_string());
    Ok((parse(width)?, parse(height)?))
}

/// The starting orientation: facing the front panorama face, tilted `pitch_degrees` down, then
/// turned `yaw_degrees` to the right.
fn initial_orientation(yaw_degrees: f64, pitch_degrees: f64) -> DQuat {
    DQuat::from_rotation_y(-yaw_degrees.to_radians())
        * DQuat::from_rotation_x(-pitch_degrees.to_radians())
}

/// How strongly the view is pulled back to right side up: the fraction of the remaining roll
/// corrected per second is `1 - e^-rate`.
const LEVELING_RATE: f64 = 1.5;

/// Shared by all monitors so they stay in sync.
struct CameraMotion {
    orientation: DQuat,
    /// World-space axis scaled by radians per second. Being fixed in the world, it keeps the
    /// view on the same path while leveling changes only the roll.
    angular_velocity: DVec3,
    last_update: Instant,
}

impl CameraMotion {
    fn advance(&mut self, now: Instant) {
        let elapsed_seconds = now
            .saturating_duration_since(self.last_update)
            .as_secs_f64();
        self.last_update = now;
        self.orientation = (DQuat::from_scaled_axis(self.angular_velocity * elapsed_seconds)
            * self.orientation)
            // Repeated multiplication slowly drifts away from unit length.
            .normalize();
        self.level(elapsed_seconds);
    }

    /// Turns `right` degrees per second around the world's vertical axis and `down` degrees per
    /// second around the camera's current level right axis.
    fn steer(&mut self, right: f64, down: f64) {
        let forward = self.orientation * DVec3::NEG_Z;
        let level_right = forward
            .cross(DVec3::Y)
            .try_normalize()
            // Looking straight up or down, the camera's own right is the only right there is.
            .unwrap_or(self.orientation * DVec3::X);
        // Turning right is a negative rotation around up, looking down a negative rotation
        // around right.
        self.angular_velocity = (DVec3::Y * -right + level_right * -down) * 1f64.to_radians();
    }

    /// Rolls the view around its forward axis, easing the horizon back to level and the view
    /// back to right side up.
    fn level(&mut self, elapsed_seconds: f64) {
        let forward = self.orientation * DVec3::NEG_Z;
        let right = self.orientation * DVec3::X;
        let level_right = forward.cross(DVec3::Y);
        // Shrinks to zero looking straight up or down, where roll is undefined; fading the
        // correction out there keeps it from spinning the view around the pole.
        let horizontal_weight = level_right.length();
        if horizontal_weight < 1e-6 {
            return;
        }
        let level_right = level_right / horizontal_weight;
        let roll_error = right
            .cross(level_right)
            .dot(forward)
            .atan2(right.dot(level_right));
        let correction =
            roll_error * horizontal_weight * (1.0 - (-LEVELING_RATE * elapsed_seconds).exp());
        self.orientation =
            (DQuat::from_axis_angle(forward, correction) * self.orientation).normalize();
    }
}

struct Monitor {
    // Declared before `layer` so the Vulkan surface is dropped before its wl_surface.
    target: Target,
    layer: LayerSurface,
    output: wl_output::WlOutput,
    logical_position: (i32, i32),
    logical_size: (u32, u32),
    scale: i32,
    /// Set from presenting until the compositor's frame callback. While set, the previous frame
    /// isn't on screen yet, or the surface is hidden and the compositor is withholding it.
    awaiting_frame_callback: bool,
}

struct App {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    connection: Connection,
    renderer: Renderer,
    monitors: Vec<Monitor>,
    arguments: Arguments,
    span: bool,
    camera_motion: CameraMotion,
    pointer: Option<wl_pointer::WlPointer>,
    cursor_shape_manager: Option<CursorShapeManager>,
    cursor_shape_device: Option<WpCursorShapeDeviceV1>,
}

impl Monitor {
    fn rectangle(&self) -> Rectangle {
        Rectangle {
            position: self.logical_position,
            size: self.logical_size,
        }
    }
}

impl App {
    /// None unless spanning.
    fn layout(&self) -> Option<Layout> {
        if !self.span {
            return None;
        }
        let rectangles: Vec<Rectangle> = self.monitors.iter().map(Monitor::rectangle).collect();
        Layout::new(&rectangles, self.arguments.fov.to_radians())
    }

    fn monitor_camera(&self, index: usize) -> Camera {
        let orientation = self.camera_motion.orientation.as_quat();
        let monitor = &self.monitors[index];
        match self.layout() {
            Some(layout) => layout.camera(monitor.rectangle(), orientation),
            None => {
                let (width, height) = monitor.logical_size;
                Camera::centered(orientation, self.arguments.fov.to_radians(), width, height)
            }
        }
    }

    fn monitor_index(&self, surface: &wl_surface::WlSurface) -> Option<usize> {
        self.monitors
            .iter()
            .position(|monitor| monitor.layer.wl_surface() == surface)
    }

    fn configure_monitor(&mut self, index: usize) {
        let monitor = &mut self.monitors[index];
        let scale = monitor.scale as u32;
        let (width, height) = monitor.logical_size;
        self.renderer
            .configure(&mut monitor.target, width * scale, height * scale);
    }

    /// Pacing comes from the timer rather than from frame callbacks, because answering every
    /// callback, even with an empty commit, keeps the compositor redrawing at the refresh rate.
    fn draw_due_monitors(&mut self, queue_handle: &QueueHandle<Self>) {
        self.camera_motion.advance(Instant::now());
        for index in 0..self.monitors.len() {
            let monitor = &self.monitors[index];
            if !monitor.awaiting_frame_callback && monitor.logical_size != (0, 0) {
                self.draw(index, queue_handle);
            }
        }
    }

    fn draw(&mut self, index: usize, queue_handle: &QueueHandle<Self>) {
        let camera = self.monitor_camera(index);
        let monitor = &mut self.monitors[index];
        let wl_surface = monitor.layer.wl_surface().clone();
        // Requested before presenting, because presenting is what commits the surface.
        wl_surface.frame(queue_handle, FrameCallbackData(wl_surface.clone()));
        monitor.awaiting_frame_callback = true;
        if !self.renderer.draw(&mut monitor.target, camera) {
            // Still commit, so that the callback arrives and the next tick retries.
            wl_surface.commit();
        }
    }

    fn add_monitor(&mut self, queue_handle: &QueueHandle<Self>, output: wl_output::WlOutput) {
        let info = self.output_state.info(&output);
        let logical_position = info
            .as_ref()
            .and_then(|info| info.logical_position)
            .unwrap_or((0, 0));
        let name = info.and_then(|info| info.name);
        if !self.arguments.outputs.is_empty()
            && !name.is_some_and(|name| self.arguments.outputs.contains(&name))
        {
            return;
        }
        let wl_surface = self.compositor.create_surface(queue_handle);
        let layer = self.layer_shell.create_layer_surface(
            queue_handle,
            wl_surface,
            Layer::Background,
            Some("panorama-paper"),
            Some(&output),
        );
        layer.set_anchor(Anchor::all());
        // -1 covers the whole output instead of avoiding bars' exclusive zones.
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_size(0, 0);
        layer.commit();

        let raw_display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
            NonNull::new(self.connection.backend().display_ptr().cast()).expect("null wl_display"),
        ));
        let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(
            NonNull::new(layer.wl_surface().id().as_ptr().cast()).expect("null wl_surface"),
        ));
        // SAFETY: the wl_display outlives the program and `Monitor` drops the wgpu surface before
        // the layer surface that owns the wl_surface.
        let surface = unsafe {
            self.renderer
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(raw_display_handle),
                    raw_window_handle,
                })
                .expect("failed to create a Vulkan surface")
        };
        self.monitors.push(Monitor {
            target: self.renderer.create_target(surface),
            layer,
            output,
            logical_position,
            logical_size: (0, 0),
            scale: 1,
            awaiting_frame_callback: false,
        });
    }
}

impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        let Some(index) = self.monitor_index(surface) else {
            return;
        };
        self.monitors[index].scale = new_factor;
        surface.set_buffer_scale(new_factor);
        if self.monitors[index].logical_size != (0, 0) {
            self.configure_monitor(index);
        }
    }

    fn transform_changed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        if let Some(index) = self.monitor_index(surface) {
            self.monitors[index].awaiting_frame_callback = false;
        }
    }

    fn surface_enter(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.add_monitor(queue_handle, output);
    }

    fn update_output(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let logical_position = self
            .output_state
            .info(&output)
            .and_then(|info| info.logical_position);
        let monitor = self
            .monitors
            .iter_mut()
            .find(|monitor| monitor.output == output);
        if let (Some(monitor), Some(logical_position)) = (monitor, logical_position) {
            monitor.logical_position = logical_position;
        }
    }

    fn output_destroyed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.monitors.retain(|monitor| monitor.output != output);
    }
}

impl LayerShellHandler for App {
    fn closed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        layer: &LayerSurface,
    ) {
        self.monitors.retain(|monitor| &monitor.layer != layer);
    }

    fn configure(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let Some(index) = self.monitor_index(layer.wl_surface()) else {
            return;
        };
        let first_configure = self.monitors[index].logical_size == (0, 0);
        if self.monitors[index].logical_size == configure.new_size {
            return;
        }
        self.monitors[index].logical_size = configure.new_size;
        self.configure_monitor(index);
        // Shows the wallpaper right away instead of on the next timer tick.
        if first_configure {
            self.camera_motion.advance(Instant::now());
            self.draw(index, queue_handle);
        }
    }
}

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability != Capability::Pointer
            || !self.arguments.follow_cursor
            || self.pointer.is_some()
        {
            return;
        }
        let pointer = self
            .seat_state
            .get_pointer(queue_handle, &seat)
            .expect("failed to get the pointer");
        self.cursor_shape_device = self
            .cursor_shape_manager
            .as_ref()
            .map(|manager| manager.get_shape_device(&pointer, queue_handle));
        self.pointer = Some(pointer);
    }

    fn remove_capability(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability != Capability::Pointer {
            return;
        }
        if let Some(cursor_shape_device) = self.cursor_shape_device.take() {
            cursor_shape_device.destroy();
        }
        if let Some(pointer) = self.pointer.take() {
            pointer.release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for App {
    fn pointer_frame(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            match event.kind {
                PointerEventKind::Enter { serial } => {
                    // Without this the cursor keeps whatever image the last window gave it.
                    if let Some(cursor_shape_device) = &self.cursor_shape_device {
                        cursor_shape_device.set_shape(serial, Shape::Default);
                    }
                }
                PointerEventKind::Motion { .. } => {}
                // Leaving keeps the current velocity, so the view keeps turning the same way.
                _ => continue,
            }
            let Some(index) = self.monitor_index(&event.surface) else {
                continue;
            };
            let monitor = &self.monitors[index];
            let (width, height) = monitor.logical_size;
            if width == 0 || height == 0 {
                continue;
            }
            // Positions relative to the screen, or to the whole layout when spanning.
            let (position, center, half_size) = match self.layout() {
                Some(layout) => (
                    (
                        f64::from(monitor.logical_position.0) + event.position.0,
                        f64::from(monitor.logical_position.1) + event.position.1,
                    ),
                    layout.center,
                    layout.half_size,
                ),
                None => {
                    let half_size = (f64::from(width) / 2.0, f64::from(height) / 2.0);
                    (event.position, half_size, half_size)
                }
            };
            let center_offset = |position: f64, center: f64, half_size: f64| {
                ((position - center) / half_size).clamp(-1.0, 1.0)
            };
            // Bank the rotation so far at the old velocity before switching to the new one.
            self.camera_motion.advance(Instant::now());
            self.camera_motion.steer(
                center_offset(position.0, center.0, half_size.0) * self.arguments.speed,
                center_offset(position.1, center.1, half_size.1) * self.arguments.speed,
            );
        }
    }
}

delegate_registry!(App);

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_dispatch2!(App);

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = Arguments::parse();
    if !arguments.fps.is_finite() || arguments.fps <= 0.0 {
        return Err("--fps must be positive".into());
    }
    let panorama = Panorama::load(&arguments.panorama)?;
    let mut renderer = Renderer::new(&panorama)?;
    drop(panorama);
    if let Some(screenshot_path) = &arguments.screenshot {
        let (width, height) = arguments.screenshot_size;
        let orientation = initial_orientation(
            arguments.speed * arguments.screenshot_time,
            f64::from(arguments.pitch),
        );
        let camera = Camera::centered(
            orientation.as_quat(),
            arguments.fov.to_radians(),
            width,
            height,
        );
        renderer
            .screenshot(width, height, camera)
            .save(screenshot_path)?;
        return Ok(());
    }

    let connection = Connection::connect_to_env()?;
    let (globals, event_queue) = registry_queue_init(&connection)?;
    let queue_handle = event_queue.handle();
    let mut app = App {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &queue_handle),
        seat_state: SeatState::new(&globals, &queue_handle),
        compositor: CompositorState::bind(&globals, &queue_handle)?,
        layer_shell: LayerShell::bind(&globals, &queue_handle)?,
        connection: connection.clone(),
        renderer,
        monitors: Vec::new(),
        camera_motion: CameraMotion {
            orientation: initial_orientation(0.0, f64::from(arguments.pitch)),
            // Spin until the cursor first touches the wallpaper.
            angular_velocity: DVec3::Y * -arguments.speed.to_radians(),
            last_update: Instant::now(),
        },
        span: !arguments.no_span && (arguments.span || arguments.outputs.is_empty()),
        arguments,
        pointer: None,
        cursor_shape_manager: CursorShapeManager::bind(&globals, &queue_handle).ok(),
        cursor_shape_device: None,
    };
    let frame_interval = Duration::from_secs_f64(1.0 / app.arguments.fps);
    let mut event_loop = EventLoop::<App>::try_new()?;
    let loop_handle = event_loop.handle();
    WaylandSource::new(connection, event_queue).insert(loop_handle.clone())?;
    loop_handle.insert_source(
        Timer::from_duration(frame_interval),
        move |deadline, _, app| {
            app.draw_due_monitors(&queue_handle);
            // Scheduled from the deadline so ticks don't drift, but never into the past, so a
            // stall doesn't cause a burst of catch-up frames.
            TimeoutAction::ToInstant((deadline + frame_interval).max(Instant::now()))
        },
    )?;
    loop {
        event_loop.dispatch(None, &mut app)?;
    }
}
