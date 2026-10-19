//! One frame of the Star Citizen window, through Windows Graphics Capture:
//! the same path OBS's window capture takes, so it works while Starlume is
//! in front and with the game borderless or windowed. A minimised game has
//! no frames to give.
//!
//! One-shot: a capture session is opened, the first frame copied to the
//! CPU, and everything closed again. Nothing keeps running between shots
//! (the idle footprint is a product feature).

use crate::Frame;

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("Star Citizen isn't running (no game window found)")]
    NoWindow,
    #[error("Star Citizen is minimised; bring it back to capture it")]
    Minimised,
    #[error("window capture needs Windows 10 (1903) or later")]
    Unsupported,
    #[error("the game window gave no frame in time")]
    Timeout,
    #[error("window capture failed: {0}")]
    Os(String),
}

/// Capture the game window now.
pub fn capture_game() -> Result<Frame, CaptureError> {
    imp::capture_game()
}

#[cfg(windows)]
mod imp {
    use std::time::{Duration, Instant};

    use windows::Graphics::Capture::{
        Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
    };
    use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
    use windows::Graphics::DirectX::DirectXPixelFormat;
    use windows::Win32::Foundation::{CloseHandle, HMODULE, HWND, LPARAM, MAX_PATH};
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
        D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    };
    use windows::Win32::Graphics::Dxgi::IDXGIDevice;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::Win32::System::WinRT::Direct3D11::{
        CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
    };
    use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    };
    use windows::core::{BOOL, Interface, PWSTR};

    use super::CaptureError;
    use crate::Frame;

    fn os(e: windows::core::Error) -> CaptureError {
        CaptureError::Os(e.message().to_string())
    }

    /// The exe name of the process owning `hwnd`.
    fn exe_of(hwnd: HWND) -> Option<String> {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
        let mut buf = [0u16; MAX_PATH as usize];
        let mut len = buf.len() as u32;
        let ok = unsafe {
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buf.as_mut_ptr()),
                &mut len,
            )
        };
        unsafe {
            let _ = CloseHandle(process);
        }
        ok.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/']).next().map(str::to_string)
    }

    /// The visible, titled top-level window of StarCitizen.exe.
    fn game_window() -> Option<HWND> {
        unsafe extern "system" fn each(hwnd: HWND, found: LPARAM) -> BOOL {
            let found = unsafe { &mut *(found.0 as *mut Option<HWND>) };
            let titled = unsafe { GetWindowTextLengthW(hwnd) } > 0;
            if titled
                && unsafe { IsWindowVisible(hwnd) }.as_bool()
                && exe_of(hwnd).is_some_and(|e| e.eq_ignore_ascii_case("StarCitizen.exe"))
            {
                *found = Some(hwnd);
                return BOOL(0);
            }
            BOOL(1)
        }
        let mut found: Option<HWND> = None;
        unsafe {
            let _ = EnumWindows(Some(each), LPARAM(&mut found as *mut _ as isize));
        }
        found
    }

    pub fn capture_game() -> Result<Frame, CaptureError> {
        if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
            return Err(CaptureError::Unsupported);
        }
        let hwnd = game_window().ok_or(CaptureError::NoWindow)?;
        if unsafe { IsIconic(hwnd) }.as_bool() {
            return Err(CaptureError::Minimised);
        }
        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
            .map_err(os)?;
        let item: GraphicsCaptureItem = unsafe { interop.CreateForWindow(hwnd) }.map_err(os)?;

        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        }
        .map_err(os)?;
        let device = device.ok_or_else(|| CaptureError::Os("no D3D11 device".into()))?;
        let context = context.ok_or_else(|| CaptureError::Os("no D3D11 context".into()))?;
        let dxgi: IDXGIDevice = device.cast().map_err(os)?;
        let d3d: IDirect3DDevice = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }
            .map_err(os)?
            .cast()
            .map_err(os)?;

        let size = item.Size().map_err(os)?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            1,
            size,
        )
        .map_err(os)?;
        let session = pool.CreateCaptureSession(&item).map_err(os)?;
        // Windows 11: no yellow capture border, no cursor. Older builds lack
        // these; the shot still works.
        let _ = session.SetIsBorderRequired(false);
        let _ = session.SetIsCursorCaptureEnabled(false);
        session.StartCapture().map_err(os)?;

        let start = Instant::now();
        let frame = loop {
            if let Ok(f) = pool.TryGetNextFrame() {
                break f;
            }
            if start.elapsed() > Duration::from_secs(2) {
                let _ = session.Close();
                let _ = pool.Close();
                return Err(CaptureError::Timeout);
            }
            std::thread::sleep(Duration::from_millis(8));
        };

        let result = (|| {
            let access: IDirect3DDxgiInterfaceAccess =
                frame.Surface().map_err(os)?.cast().map_err(os)?;
            let texture: ID3D11Texture2D = unsafe { access.GetInterface() }.map_err(os)?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            unsafe { texture.GetDesc(&mut desc) };
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            desc.MiscFlags = 0;
            let mut staging: Option<ID3D11Texture2D> = None;
            unsafe { device.CreateTexture2D(&desc, None, Some(&mut staging)) }.map_err(os)?;
            let staging = staging.ok_or_else(|| CaptureError::Os("no staging texture".into()))?;
            unsafe { context.CopyResource(&staging, &texture) };
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }
                .map_err(os)?;
            let (w, h) = (desc.Width, desc.Height);
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                let row = unsafe {
                    std::slice::from_raw_parts(
                        (mapped.pData as *const u8).add((y * mapped.RowPitch) as usize),
                        (w * 4) as usize,
                    )
                };
                for px in row.as_chunks::<4>().0 {
                    rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
                }
            }
            unsafe { context.Unmap(&staging, 0) };
            Ok(Frame {
                width: w,
                height: h,
                rgba,
            })
        })();
        let _ = frame.Close();
        let _ = session.Close();
        let _ = pool.Close();
        result
    }
}

#[cfg(not(windows))]
mod imp {
    use super::CaptureError;
    use crate::Frame;

    pub fn capture_game() -> Result<Frame, CaptureError> {
        Err(CaptureError::Unsupported)
    }
}
