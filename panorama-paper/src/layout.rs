use crate::renderer::Camera;
use glam::Quat;

/// A monitor's area in the compositor's logical coordinates (y grows downwards).
#[derive(Clone, Copy)]
pub struct Rectangle {
    pub position: (i32, i32),
    pub size: (u32, u32),
}

impl Rectangle {
    fn edges(self) -> (f64, f64, f64, f64) {
        let (x, y) = (f64::from(self.position.0), f64::from(self.position.1));
        (x, y, x + f64::from(self.size.0), y + f64::from(self.size.1))
    }
}

/// The bounding box of the drawn monitors, treated as one screen.
pub struct Layout {
    pub center: (f64, f64),
    pub half_size: (f64, f64),
    /// In logical pixels, so that the vertical field of view holds across the reference
    /// monitor's height.
    focal_length: f64,
}

impl Layout {
    pub fn new(rectangles: &[Rectangle], vertical_fov: f32) -> Option<Self> {
        let placed: Vec<Rectangle> = rectangles
            .iter()
            .copied()
            .filter(|rectangle| rectangle.size.0 > 0 && rectangle.size.1 > 0)
            .collect();
        let (left, top, right, bottom) = placed.iter().map(|rectangle| rectangle.edges()).reduce(
            |(left, top, right, bottom), (other_left, other_top, other_right, other_bottom)| {
                (
                    left.min(other_left),
                    top.min(other_top),
                    right.max(other_right),
                    bottom.max(other_bottom),
                )
            },
        )?;
        // A portrait monitor's height would make the view far too zoomed out on the others.
        let landscape_heights = placed
            .iter()
            .filter(|rectangle| rectangle.size.0 >= rectangle.size.1)
            .map(|rectangle| rectangle.size.1);
        let all_heights = placed.iter().map(|rectangle| rectangle.size.1);
        let reference_height = landscape_heights.max().or(all_heights.max())?;
        Some(Self {
            center: ((left + right) / 2.0, (top + bottom) / 2.0),
            half_size: ((right - left) / 2.0, (bottom - top) / 2.0),
            focal_length: f64::from(reference_height) / 2.0 / f64::from(vertical_fov / 2.0).tan(),
        })
    }

    /// The camera showing `rectangle`'s slice of one flat image plane spanning the layout.
    pub fn camera(&self, rectangle: Rectangle, orientation: Quat) -> Camera {
        let half_width = f64::from(rectangle.size.0) / 2.0;
        let half_height = f64::from(rectangle.size.1) / 2.0;
        let center_x = f64::from(rectangle.position.0) + half_width;
        let center_y = f64::from(rectangle.position.1) + half_height;
        Camera {
            orientation,
            // Logical y grows downwards, the image surface's y upwards.
            offset: [
                ((center_x - self.center.0) / self.focal_length) as f32,
                ((self.center.1 - center_y) / self.focal_length) as f32,
            ],
            half_extent: [
                (half_width / self.focal_length) as f32,
                (half_height / self.focal_length) as f32,
            ],
        }
    }
}
