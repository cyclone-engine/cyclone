use crate::protocol::{MessageKind, Packet, WireDelta, WireSnapshot};
use crate::replication::apply;
use crate::snapshot::{Snapshot, SnapshotDelta};

/// Đếm nội bộ, không throw mỗi frame — game không bắt buộc đọc. Không nuốt
/// lỗi hoàn toàn im lặng: có chỗ để kiểm tra "có bất thường không" nếu cần,
/// mà không phải đổi API `SnapshotCache::update()`/`current()`.
///
/// Gộp thành 1 struct riêng (thay vì field `u64` rời trong `SnapshotCache`)
/// để có sẵn chỗ mở rộng khi cần — TODO (roadmap, v0.6+): expose qua
/// `GameClient::diagnostics()` khi có nhu cầu thật (xem
/// docs/versions/v0.5/design.md), chưa expose ở v0.5.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ClientDiagnostics {
    pub decode_errors: u64,
    pub packets_received: u64,
}

/// Tái tạo Snapshot hiện hành từ luồng `Packet` nhận được — game không tự
/// giữ baseline hay tự gọi `apply()`. Đối xứng `SnapshotSender` phía gửi
/// (v0.3/v0.4): đây chính là "SnapshotReceiver" mà TODO trong
/// `replication/sender.rs` đã nhắc tới.
pub(crate) struct SnapshotCache {
    current: Option<Snapshot>,
    diagnostics: ClientDiagnostics,
}

impl SnapshotCache {
    pub(crate) fn new() -> Self {
        Self {
            current: None,
            diagnostics: ClientDiagnostics::default(),
        }
    }

    /// Decode lỗi không throw lên game mỗi frame — giữ nguyên state cũ,
    /// chỉ đếm lại (xem "Decode lỗi", docs/versions/v0.5/design.md).
    pub(crate) fn update(&mut self, packet: Packet) {
        self.diagnostics.packets_received += 1;
        match packet.kind {
            MessageKind::Snapshot => match WireSnapshot::from_bytes(&packet.payload) {
                Ok(wire) => self.current = Some(Snapshot::from(&wire)),
                Err(_) => self.diagnostics.decode_errors += 1,
            },
            MessageKind::Delta => match WireDelta::from_bytes(&packet.payload) {
                Ok(wire) => {
                    let delta = SnapshotDelta::from(&wire);
                    // Nhận Delta trước khi có baseline (Snapshot đầu tiên
                    // chưa tới) -> bỏ qua, không có gì để apply lên. Không
                    // tính là lỗi decode (payload hợp lệ).
                    if let Some(old) = &self.current {
                        self.current = Some(apply(old, &delta));
                    }
                }
                Err(_) => self.diagnostics.decode_errors += 1,
            },
            // Server không bao giờ gửi Input — chiều đó chỉ client -> server.
            MessageKind::Input => {}
        }
    }

    pub(crate) fn current(&self) -> Option<&Snapshot> {
        self.current.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn diagnostics(&self) -> ClientDiagnostics {
        self.diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ProtocolVersion;

    fn packet(kind: MessageKind, payload: Vec<u8>) -> Packet {
        Packet {
            version: ProtocolVersion::CURRENT,
            kind,
            payload,
        }
    }

    #[test]
    fn garbage_payload_is_counted_not_thrown() {
        let mut cache = SnapshotCache::new();
        cache.update(packet(MessageKind::Snapshot, vec![1, 2, 3])); // quá ngắn để decode

        let diag = cache.diagnostics();
        assert_eq!(diag.decode_errors, 1);
        assert_eq!(diag.packets_received, 1);
        assert!(cache.current().is_none());
    }

    #[test]
    fn delta_before_baseline_is_ignored_not_counted_as_error() {
        let mut cache = SnapshotCache::new();
        let empty_delta = crate::snapshot::SnapshotDelta {
            tick: crate::time::TickId(0),
            items: Vec::new(),
        };
        let wire = WireDelta::from(&empty_delta);
        cache.update(packet(MessageKind::Delta, wire.to_bytes()));

        let diag = cache.diagnostics();
        assert_eq!(diag.decode_errors, 0);
        assert_eq!(diag.packets_received, 1);
        assert!(cache.current().is_none());
    }
}
