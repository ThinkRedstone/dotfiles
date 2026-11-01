use crate::panorama::Panorama;
use glam::Quat;
use image::RgbaImage;
use std::error::Error;

/// Cubemap layers are ordered +X, -X, +Y, -Y, +Z, -Z; these are the matching panorama faces
/// (right, left, up, down, front, back).
const CUBE_LAYER_FACES: [usize; 6] = [1, 3, 4, 5, 0, 2];
const CAMERA_UNIFORM_SIZE: u64 = 32;

/// The view orientation, plus which part of the image surface a target shows. `offset` and
/// `half_extent` are in units of the focal length on a flat image plane, with y pointing up.
#[derive(Clone, Copy)]
pub struct Camera {
    /// Rotates camera space (x right, y up, looking down -z) into world space.
    pub orientation: Quat,
    pub offset: [f32; 2],
    pub half_extent: [f32; 2],
}

impl Camera {
    /// A flat view centered on the screen.
    pub fn centered(orientation: Quat, vertical_fov: f32, width: u32, height: u32) -> Self {
        let half_height = (vertical_fov / 2.0).tan();
        Self {
            orientation,
            offset: [0.0, 0.0],
            half_extent: [half_height * width as f32 / height as f32, half_height],
        }
    }
}

pub struct Renderer {
    pub instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    panorama_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    /// Built on the first configure, since the surface format is unknown until then.
    pipeline: Option<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
}

/// Each render target gets its own camera uniform, since the aspect ratio differs per monitor.
struct CameraBinding {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// Per-monitor GPU state.
pub struct Target {
    pub surface: wgpu::Surface<'static>,
    configuration: Option<wgpu::SurfaceConfiguration>,
    camera_binding: CameraBinding,
}

impl Renderer {
    pub fn new(panorama: &Panorama) -> Result<Self, Box<dyn Error>> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;

        let face_extent = wgpu::Extent3d {
            width: panorama.face_size,
            height: panorama.face_size,
            depth_or_array_layers: 1,
        };
        let panorama_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("panorama"),
            size: wgpu::Extent3d {
                depth_or_array_layers: 6,
                ..face_extent
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (layer, &face_index) in CUBE_LAYER_FACES.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &panorama_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                panorama.faces[face_index].as_raw(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * panorama.face_size),
                    rows_per_image: Some(panorama.face_size),
                },
                face_extent,
            );
        }
        let panorama_view = panorama_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("panorama"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("panorama"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            bind_group_layout,
            pipeline_layout,
            shader,
            panorama_view,
            sampler,
            pipeline: None,
        })
    }

    fn create_camera_binding(&self) -> CameraBinding {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: CAMERA_UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("panorama"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.panorama_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer.as_entire_binding(),
                },
            ],
        });
        CameraBinding { buffer, bind_group }
    }

    pub fn create_target(&self, surface: wgpu::Surface<'static>) -> Target {
        Target {
            surface,
            configuration: None,
            camera_binding: self.create_camera_binding(),
        }
    }

    /// `width` and `height` are in physical pixels.
    pub fn configure(&mut self, target: &mut Target, width: u32, height: u32) {
        let mut configuration = target
            .surface
            .get_default_config(&self.adapter, width, height)
            .expect("surface is not supported by the adapter");
        let capabilities = target.surface.get_capabilities(&self.adapter);
        // Mailbox never blocks on present, so one monitor's vsync can't stall the others, which
        // share this thread; pacing comes from Wayland frame callbacks instead.
        if capabilities
            .present_modes
            .contains(&wgpu::PresentMode::Mailbox)
        {
            configuration.present_mode = wgpu::PresentMode::Mailbox;
        }
        match &self.pipeline {
            Some((format, _)) if capabilities.formats.contains(format) => {
                configuration.format = *format
            }
            Some(_) => panic!("monitors with incompatible surface formats are not supported"),
            None => self.ensure_pipeline(configuration.format),
        }
        target.surface.configure(&self.device, &configuration);
        target.configuration = Some(configuration);
    }

    fn ensure_pipeline(&mut self, format: wgpu::TextureFormat) {
        if self.pipeline.is_none() {
            self.pipeline = Some((format, self.create_pipeline(format)));
        }
    }

    fn create_pipeline(&self, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("panorama"),
                layout: Some(&self.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vertex_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fragment_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(format.into())],
                }),
                multiview_mask: None,
                cache: None,
            })
    }

    /// Returns false when no frame was presented, in which case the caller must commit the
    /// surface itself.
    pub fn draw(&mut self, target: &mut Target, camera: Camera) -> bool {
        let Some(configuration) = &target.configuration else {
            return false;
        };
        let surface_texture = match target.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                let (width, height) = (configuration.width, configuration.height);
                self.configure(target, width, height);
                return false;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return false;
            }
            wgpu::CurrentSurfaceTexture::Validation => panic!("surface validation error"),
        };

        let view = surface_texture.texture.create_view(&Default::default());
        let encoder = self.encode_frame(&target.camera_binding, &view, camera);
        self.queue.submit([encoder.finish()]);
        self.queue.present(surface_texture);
        true
    }

    fn encode_frame(
        &self,
        camera_binding: &CameraBinding,
        view: &wgpu::TextureView,
        camera: Camera,
    ) -> wgpu::CommandEncoder {
        let camera_uniform: Vec<u8> = [
            camera.orientation.x,
            camera.orientation.y,
            camera.orientation.z,
            camera.orientation.w,
            camera.half_extent[0],
            camera.half_extent[1],
            camera.offset[0],
            camera.offset[1],
        ]
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect();
        self.queue
            .write_buffer(&camera_binding.buffer, 0, &camera_uniform);

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("panorama"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let (_, pipeline) = self
                .pipeline
                .as_ref()
                .expect("pipeline is built before drawing");
            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, &camera_binding.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
        encoder
    }

    /// Renders a single frame offscreen, for previewing settings without a compositor.
    pub fn screenshot(&mut self, width: u32, height: u32, camera: Camera) -> RgbaImage {
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        self.ensure_pipeline(format);
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("screenshot"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let padded_bytes_per_row = (4 * width).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot"),
            size: u64::from(padded_bytes_per_row * height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_binding = self.create_camera_binding();
        let view = texture.create_view(&Default::default());
        let mut encoder = self.encode_frame(&camera_binding, &view, camera);
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            extent,
        );
        self.queue.submit([encoder.finish()]);
        readback_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, |result| {
                result.expect("failed to map screenshot")
            });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .expect("failed to wait for the GPU");

        let mapped = readback_buffer
            .slice(..)
            .get_mapped_range()
            .expect("screenshot buffer is mapped");
        let pixels = mapped
            .chunks(padded_bytes_per_row as usize)
            .flat_map(|row| &row[..4 * width as usize])
            .copied()
            .collect();
        RgbaImage::from_raw(width, height, pixels).expect("buffer matches the image size")
    }
}
