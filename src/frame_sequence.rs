use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::scene::SceneConfig;

#[derive(Debug, Serialize)]
pub struct FrameSequence {
    pub pattern: &'static str,
    pub start_number: u32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frames: u32,
    pub duration: f64,
    pub alpha: &'static str,
    pub color_space: &'static str,
}

#[derive(Debug)]
pub struct FrameSequenceError {
    pub file: PathBuf,
    pub source: io::Error,
}

pub struct FrameSequenceWriter {
    directory: PathBuf,
    owned: Vec<PathBuf>,
    scene_config: SceneConfig,
    frames: u32,
    finished: bool,
}

impl FrameSequenceWriter {
    pub fn start(scene_config: &SceneConfig, directory: &Path) -> Result<Self, FrameSequenceError> {
        fs::create_dir(directory).map_err(|source| FrameSequenceError {
            file: directory.to_path_buf(),
            source,
        })?;
        let mut writer = Self {
            directory: directory.to_path_buf(),
            owned: Vec::new(),
            scene_config: *scene_config,
            frames: 0,
            finished: false,
        };
        writer.create_file(directory.join(".incomplete"))?;
        Ok(writer)
    }

    /// The file the frame at `index` goes in.
    pub fn file(&self, index: u32) -> PathBuf {
        self.directory.join(format!("frame-{index:06}.png"))
    }

    /// `png` is the next frame, encoded.
    pub fn push_png(&mut self, png: &[u8]) -> Result<(), FrameSequenceError> {
        let path = self.file(self.frames);
        let mut file = self.create_file(path.clone())?;
        file.write_all(png)
            .map_err(|source| FrameSequenceError { file: path, source })?;
        self.frames += 1;
        Ok(())
    }

    pub fn finish(mut self) -> Result<FrameSequence, FrameSequenceError> {
        let incomplete_file = self.directory.join(".incomplete");
        fs::remove_file(&incomplete_file).map_err(|source| FrameSequenceError {
            file: incomplete_file,
            source,
        })?;
        self.finished = true;
        let fps = self.scene_config.fps.get();
        Ok(FrameSequence {
            pattern: "frame-%06d.png",
            start_number: 0,
            width: self.scene_config.width.get(),
            height: self.scene_config.height.get(),
            fps,
            frames: self.frames,
            duration: f64::from(self.frames) / f64::from(fps),
            alpha: "straight",
            color_space: "srgb",
        })
    }

    fn create_file(&mut self, path: PathBuf) -> Result<File, FrameSequenceError> {
        let file = File::create_new(&path).map_err(|source| FrameSequenceError {
            file: path.clone(),
            source,
        })?;
        self.owned.push(path);
        Ok(file)
    }
}

impl Drop for FrameSequenceWriter {
    fn drop(&mut self) {
        if !self.finished {
            for path in self.owned.iter().rev() {
                let _ = fs::remove_file(path);
            }
            let _ = fs::remove_dir(&self.directory);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Frame;

    #[test]
    fn a_numbered_file_created_by_someone_else_survives_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let directory = dir.path().join("frames");
        let mut writer = FrameSequenceWriter::start(&SceneConfig::default(), &directory).unwrap();
        let frame = Frame {
            width: 1,
            height: 1,
            rgba: vec![12, 34, 56, 128],
        };
        writer.push_png(&frame.png().unwrap()).unwrap();
        let occupied = directory.join("frame-000001.png");
        fs::write(&occupied, b"another process owns this").unwrap();
        let unrelated = directory.join("notes.txt");
        fs::write(&unrelated, b"keep these notes").unwrap();

        let error = writer.push_png(&frame.png().unwrap()).unwrap_err();
        drop(writer);

        assert_eq!(error.file, occupied);
        assert_eq!(error.source.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&occupied).unwrap(), b"another process owns this");
        assert_eq!(fs::read(&unrelated).unwrap(), b"keep these notes");
        assert_eq!(fs::read_dir(directory).unwrap().count(), 2);
    }
}
