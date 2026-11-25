//! RFCOMM transport over Winsock (AF_BTH), connecting by channel number like
//! BoseBuddy's RFCOMMLink on macOS.
//!
//! The socket is non-blocking and polled: Windows documents that after a
//! SO_RCVTIMEO timeout "the socket state is indeterminate, and should not be
//! used", so blocking reads with a timeout are avoided.

use std::time::{Duration, Instant};

use windows::Win32::Devices::Bluetooth::{AF_BTH, BTHPROTO_RFCOMM, SOCKADDR_BTH};
use windows::Win32::Networking::WinSock::{
    closesocket, connect, ioctlsocket, recv, send, socket, WSACleanup, WSAGetLastError,
    WSAStartup, FIONBIO, INVALID_SOCKET, SEND_RECV_FLAGS, SOCKADDR, SOCKET, SOCK_STREAM,
    WSADATA, WSAEWOULDBLOCK,
};

pub struct Rfcomm {
    sock: SOCKET,
    pub channel: u8,
}

impl Rfcomm {
    /// Connect to `address` (u64, e.g. 0x782B645B71EE), trying each channel.
    pub fn open(address: u64, channels: &[u8]) -> Result<Rfcomm, String> {
        let mut data = WSADATA::default();
        let rc = unsafe { WSAStartup(0x0202, &mut data) };
        if rc != 0 {
            return Err(format!("WSAStartup failed: {rc}"));
        }
        let mut errors = Vec::new();
        for &channel in channels {
            match Self::open_channel(address, channel) {
                Ok(sock) => return Ok(Rfcomm { sock, channel }),
                Err(e) => errors.push(format!("channel {channel}: {e}")),
            }
        }
        unsafe { WSACleanup() };
        Err(errors.join("; "))
    }

    fn open_channel(address: u64, channel: u8) -> Result<SOCKET, String> {
        unsafe {
            let sock = socket(AF_BTH as i32, SOCK_STREAM, BTHPROTO_RFCOMM as i32)
                .map_err(|e| format!("socket: {e}"))?;
            if sock == INVALID_SOCKET {
                return Err(format!("socket: {:?}", WSAGetLastError()));
            }
            let addr = SOCKADDR_BTH {
                addressFamily: AF_BTH,
                btAddr: address,
                serviceClassId: Default::default(),
                port: channel as u32,
            };
            let rc = connect(
                sock,
                &addr as *const SOCKADDR_BTH as *const SOCKADDR,
                std::mem::size_of::<SOCKADDR_BTH>() as i32,
            );
            if rc != 0 {
                let err = WSAGetLastError();
                let _ = closesocket(sock);
                return Err(format!("connect: {err:?}"));
            }
            let mut nonblocking: u32 = 1;
            if ioctlsocket(sock, FIONBIO, &mut nonblocking) != 0 {
                let err = WSAGetLastError();
                let _ = closesocket(sock);
                return Err(format!("ioctlsocket: {err:?}"));
            }
            Ok(sock)
        }
    }

    /// Bytes waiting right now, without blocking. `Err` = connection closed or failed.
    fn read_available(&self, into: &mut Vec<u8>) -> Result<usize, String> {
        let mut buf = [0u8; 1024];
        let mut total = 0;
        loop {
            let n = unsafe { recv(self.sock, &mut buf, SEND_RECV_FLAGS(0)) };
            if n > 0 {
                into.extend_from_slice(&buf[..n as usize]);
                total += n as usize;
                continue;
            }
            if n == 0 {
                return Err("connection closed by the headset".into());
            }
            let err = unsafe { WSAGetLastError() };
            if err == WSAEWOULDBLOCK {
                return Ok(total);
            }
            return Err(format!("recv: {err:?}"));
        }
    }

    /// Send one packet and collect the reply.
    ///
    /// `drain = false`: stop as soon as the buffer holds whole packets.
    /// `drain = true`: keep reading until 300 ms pass with no new data (GetAll,
    /// START streams). Mirrors RFCOMMLink.sendSync on macOS. Stale bytes left
    /// from an earlier request are discarded first and returned separately.
    pub fn exchange(&self, packet: &[u8], drain: bool, timeout: Duration)
        -> Result<(Vec<u8>, Vec<u8>), String> {
        let mut stale = Vec::new();
        self.read_available(&mut stale)?;

        let sent = unsafe { send(self.sock, packet, SEND_RECV_FLAGS(0)) };
        if sent != packet.len() as i32 {
            return Err(format!("send: {:?}", unsafe { WSAGetLastError() }));
        }

        let deadline = Instant::now() + timeout;
        let mut buffer = Vec::new();
        let mut last_data = Instant::now();
        while Instant::now() < deadline {
            if self.read_available(&mut buffer)? > 0 {
                last_data = Instant::now();
                if !drain && crate::bmap::is_complete(&buffer) {
                    break;
                }
            }
            if drain && !buffer.is_empty() && last_data.elapsed() > Duration::from_millis(300) {
                break;
            }
            std::thread::sleep(Duration::from_millis(8));
        }
        if buffer.is_empty() {
            return Err(format!("timeout after {:.1}s", timeout.as_secs_f32()));
        }
        Ok((buffer, stale))
    }

    /// Send without waiting for a reply (DISCONNECT is fire-and-forget on macOS).
    pub fn send_only(&self, packet: &[u8]) -> Result<(), String> {
        let sent = unsafe { send(self.sock, packet, SEND_RECV_FLAGS(0)) };
        if sent != packet.len() as i32 {
            return Err(format!("send: {:?}", unsafe { WSAGetLastError() }));
        }
        std::thread::sleep(Duration::from_millis(120));
        let mut ignored = Vec::new();
        let _ = self.read_available(&mut ignored);
        Ok(())
    }
}

impl Drop for Rfcomm {
    fn drop(&mut self) {
        unsafe {
            let _ = closesocket(self.sock);
            WSACleanup();
        }
    }
}
