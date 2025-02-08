use super::model::Capturer;
use crate::duplication_context::DuplicationContext;
use crate::utils::OutDuplDescExt;
use crate::Error;
use crate::Result;
use windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC;
use windows::Win32::Graphics::Dxgi::DXGI_OUTDUPL_POINTER_SHAPE_INFO;
use windows::Win32::Graphics::{
  Direct3D11::ID3D11Texture2D,
  Dxgi::{DXGI_OUTDUPL_FRAME_INFO, DXGI_OUTPUT_DESC},
};

/// Capture screen to a chunk of memory.
pub struct CustomCapturer<'a> {
  buffer: &'a mut [u8],
  ctx: &'a DuplicationContext,
  texture: ID3D11Texture2D,
  texture_desc: D3D11_TEXTURE2D_DESC,
  pointer_shape_buffer: Vec<u8>,
  pointer_shape_buffer_size: usize,
}

impl<'a> CustomCapturer<'a> {
  pub fn with_texture(
    ctx: &'a DuplicationContext,
    buffer: &'a mut [u8],
    texture: ID3D11Texture2D,
    texture_desc: D3D11_TEXTURE2D_DESC,
  ) -> Self {
    Self {
      buffer,
      ctx,
      texture,
      texture_desc,
      pointer_shape_buffer: Vec::new(),
      pointer_shape_buffer_size: 0,
    }
  }

  pub fn new(ctx: &'a DuplicationContext, buffer: &'a mut [u8]) -> Result<Self> {
    let (texture, _desc, texture_desc) = ctx.create_readable_texture()?;
    Ok(Self::with_texture(ctx, buffer, texture, texture_desc))
  }
}

impl Capturer for CustomCapturer<'_> {
  fn dxgi_output_desc(&self) -> Result<DXGI_OUTPUT_DESC> {
    self.ctx.dxgi_output_desc()
  }

  fn dxgi_outdupl_desc(&self) -> windows::Win32::Graphics::Dxgi::DXGI_OUTDUPL_DESC {
    self.ctx.dxgi_outdupl_desc()
  }

  fn buffer(&self) -> &[u8] {
    self.buffer
  }

  fn buffer_mut(&mut self) -> &mut [u8] {
    self.buffer
  }

  fn check_buffer(&self) -> Result<()> {
    if self.buffer.len() < self.dxgi_outdupl_desc().calc_buffer_size() {
      Err(Error::InvalidBufferLength)
    } else {
      Ok(())
    }
  }

  fn pointer_shape_buffer(&self) -> &[u8] {
    &self.pointer_shape_buffer[..self.pointer_shape_buffer_size]
  }

  fn capture(&mut self) -> Result<DXGI_OUTDUPL_FRAME_INFO> {
    self.ctx.capture(
      self.buffer.as_mut_ptr(),
      self.buffer.len(),
      &self.texture,
      &self.texture_desc,
    )
  }

  fn safe_capture(&mut self) -> Result<DXGI_OUTDUPL_FRAME_INFO> {
    self.check_buffer()?;
    self.capture()
  }

  fn capture_with_pointer_shape(
    &mut self,
  ) -> Result<(
    DXGI_OUTDUPL_FRAME_INFO,
    Option<DXGI_OUTDUPL_POINTER_SHAPE_INFO>,
  )> {
    let (frame_info, pointer_shape_info) = self.ctx.capture_with_pointer_shape(
      self.buffer.as_mut_ptr(),
      self.buffer.len(),
      &self.texture,
      &self.texture_desc,
      &mut self.pointer_shape_buffer,
    )?;

    if pointer_shape_info.is_some() {
      // record the pointer shape buffer size
      self.pointer_shape_buffer_size = frame_info.PointerShapeBufferSize as usize;
    }

    Ok((frame_info, pointer_shape_info))
  }

  fn safe_capture_with_pointer_shape(
    &mut self,
  ) -> Result<(
    DXGI_OUTDUPL_FRAME_INFO,
    Option<DXGI_OUTDUPL_POINTER_SHAPE_INFO>,
  )> {
    self.check_buffer()?;
    self.capture_with_pointer_shape()
  }
}

impl DuplicationContext {
  pub fn custom_capturer<'a>(&'a self, buffer: &'a mut [u8]) -> Result<CustomCapturer<'a>> {
    CustomCapturer::<'a>::new(self, buffer)
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    capturer::model::Capturer,
    manager::Manager,
    utils::{FrameInfoExt, OutDuplDescExt},
  };
  use serial_test::serial;
  use std::{thread, time::Duration};

  #[test]
  #[serial]
  fn custom_capturer() {
    let mut manager = Manager::default();
    manager.refresh().unwrap();
    assert_ne!(manager.contexts.len(), 0);

    let ctx = &manager.contexts[0];
    let desc = ctx.dxgi_outdupl_desc();
    let mut buffer = vec![0u8; desc.calc_buffer_size()];
    let mut capturer = ctx.custom_capturer(&mut buffer).unwrap();

    // sleep for a while before capture to wait system to update the screen
    thread::sleep(Duration::from_millis(100));

    let info = capturer.safe_capture().unwrap();
    assert!(info.desktop_updated());

    let buffer = capturer.buffer();
    // ensure buffer not all zero
    let mut all_zero = true;
    for i in 0..buffer.len() {
      if buffer[i] != 0 {
        all_zero = false;
        break;
      }
    }
    assert!(!all_zero);

    // sleep for a while before capture to wait system to update the mouse
    thread::sleep(Duration::from_millis(1000));

    // check pointer shape
    let (frame_info, pointer_shape_info) = capturer.safe_capture_with_pointer_shape().unwrap();
    assert!(frame_info.mouse_updated().position_updated);
    assert!(pointer_shape_info.is_some());
    let pointer_shape_data = capturer.pointer_shape_buffer();
    // make sure pointer shape buffer is not all zero
    let mut all_zero = true;
    for i in 0..pointer_shape_data.len() {
      if pointer_shape_data[i] != 0 {
        all_zero = false;
        break;
      }
    }
    assert!(!all_zero);
  }
}
