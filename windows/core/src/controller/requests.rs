//! Requests to the headset: one at a time through the link, with only the
//! reply to *this* request accepted, failures logged once in plain English,
//! and an optional hex trace.

use super::*;

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_millis(2800);

impl Controller {
    pub(super) fn begin(&mut self, action: &str) {
        self.state.busy = Some(action.to_string());
        self.changed();
    }

    pub(super) fn end(&mut self) {
        self.state.busy = None;
    }

    pub(super) fn pause(&mut self, duration: Duration) {
        if let Some(link) = self.link.as_mut() {
            link.pause(duration);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn exchange(
        &mut self,
        fblock: u8,
        function: u8,
        op: Op,
        payload: &[u8],
        expect_reply: bool,
        drain: bool,
        timeout: Duration,
    ) -> Result<Vec<Packet>, BmapError> {
        let Some(link) = self.link.as_mut() else { return Err(BmapError::NotConnected) };
        let request = Packet::new(fblock, function, op, payload).encode();
        let started = Instant::now();
        let result = link.exchange(&request, expect_reply, drain, timeout);
        if self.trace {
            self.log.write(format!("TX {fblock}.{function} {op} [{}]", hex(payload)));
        }
        match result {
            Ok(bytes) => {
                let packets = parse_all(&bytes);
                if self.trace {
                    for p in &packets {
                        self.log.write(format!("RX {}.{} {} [{}]", p.fblock, p.function, p.op, hex(&p.payload)));
                    }
                }
                if let Some(reply) = packets.iter().find(|p| p.is(fblock, function) && p.op == Op::Error) {
                    let code = reply.payload.first().copied().unwrap_or(0);
                    self.log_failure(request_description::device_error(fblock, function, op, code));
                }
                Ok(packets)
            }
            Err(error) => {
                let seconds = started.elapsed().as_secs_f64();
                self.log_failure(request_description::failure(fblock, function, op, &error, seconds));
                if error == BmapError::NotConnected {
                    self.handle_control_closed();
                }
                Err(error)
            }
        }
    }

    pub(super) fn get(&mut self, fblock: u8, function: u8, payload: &[u8]) -> Result<Packet, BmapError> {
        let packets = self.exchange(fblock, function, Op::Get, payload, true, false, DEFAULT_TIMEOUT)?;
        // Only the reply to this request counts; anything else (a notification,
        // a late reply to another request) isn't this answer. BMAP has no
        // request IDs, so a late reply to the *same* request can't be told
        // apart; the transport drops leftovers before each send for that.
        let Some(reply) = reply_to(&packets, fblock, function).cloned() else {
            self.log_failure(request_description::failure(fblock, function, Op::Get, &BmapError::NoResponse, 0.0));
            return Err(BmapError::NoResponse);
        };
        if reply.op == Op::Error {
            return Err(BmapError::Device { code: reply.payload.first().copied().unwrap_or(0), fblock, function });
        }
        Ok(reply)
    }

    pub(super) fn log_failure(&mut self, line: String) {
        if self.logged_failures.insert(failure_key(&line)) {
            self.log.write(line);
        }
    }
}

/// A failure line without its duration (" after 2.8s"), so the same failure
/// with a different timing counts as already logged.
pub(crate) fn failure_key(line: &str) -> String {
    if let Some(start) = line.find(" after ") {
        let rest = &line[start + " after ".len()..];
        let digits = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
        if digits > 0 && rest[digits..].starts_with('s') {
            return format!("{}{}", &line[..start], &rest[digits + 1..]);
        }
    }
    line.to_string()
}

/// A write succeeded only if the headset answered it without an error.
pub(super) fn ensure_ok(packets: &[Packet], fblock: u8, function: u8) -> Result<(), BmapError> {
    match reply_to(packets, fblock, function) {
        Some(reply) if reply.op == Op::Error => {
            Err(BmapError::Device { code: reply.payload.first().copied().unwrap_or(0), fblock, function })
        }
        Some(_) => Ok(()),
        None => Err(BmapError::NoResponse),
    }
}
