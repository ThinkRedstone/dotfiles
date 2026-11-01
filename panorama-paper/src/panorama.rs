use image::{ImageFormat, RgbaImage};
use std::{error::Error, fs::File, io::Read, path::Path};

const PACK_BACKGROUND_DIRECTORY: &str = "assets/minecraft/textures/gui/title/background";

/// The six Minecraft panorama faces, indexed like the files: 0 front, 1 right, 2 back, 3 left,
/// 4 up, 5 down.
pub struct Panorama {
    pub faces: Vec<RgbaImage>,
    pub face_size: u32,
}

impl Panorama {
    /// Loads from a resource-pack zip, a resource-pack directory, or a directory that directly
    /// holds `panorama_0.png` … `panorama_5.png`.
    pub fn load(path: &Path) -> Result<Self, Box<dyn Error>> {
        let encoded_faces = if path.is_dir() {
            read_directory(path)?
        } else {
            read_zip(path)?
        };
        let faces = encoded_faces
            .iter()
            .map(|bytes| {
                image::load_from_memory_with_format(bytes, ImageFormat::Png)
                    .map(|face| face.into_rgba8())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let face_size = faces[0].width();
        if faces
            .iter()
            .any(|face| face.width() != face_size || face.height() != face_size)
        {
            return Err("panorama faces must all be the same square size".into());
        }
        Ok(Self { faces, face_size })
    }
}

fn face_file_name(index: usize) -> String {
    format!("panorama_{index}.png")
}

fn read_directory(directory: &Path) -> Result<Vec<Vec<u8>>, Box<dyn Error>> {
    let background_directory = if directory.join(face_file_name(0)).is_file() {
        directory.to_path_buf()
    } else {
        directory.join(PACK_BACKGROUND_DIRECTORY)
    };
    (0..6)
        .map(|index| {
            let face_path = background_directory.join(face_file_name(index));
            std::fs::read(&face_path)
                .map_err(|error| format!("{}: {error}", face_path.display()).into())
        })
        .collect()
}

fn read_zip(zip_path: &Path) -> Result<Vec<Vec<u8>>, Box<dyn Error>> {
    let mut archive = zip::ZipArchive::new(File::open(zip_path)?)?;
    (0..6)
        .map(|index| {
            let file_name = face_file_name(index);
            let entry_name = archive
                .file_names()
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|name| !name.starts_with("__MACOSX/"))
                .find(|name| *name == file_name || name.ends_with(&format!("/{file_name}")))
                .ok_or_else(|| format!("{file_name} not found in {}", zip_path.display()))?
                .into_owned();
            let mut bytes = Vec::new();
            archive.by_name(&entry_name)?.read_to_end(&mut bytes)?;
            Ok(bytes)
        })
        .collect()
}
