//! HebEvent — the message type that rides the leader-election bus.

use swissarmyhammer_leader_election::{BusMessage, ElectionError};

use crate::header::EventHeader;

/// HEB's message type — header + body envelope.
#[derive(Debug, Clone)]
pub struct HebEvent {
    pub header: EventHeader,
    pub body: Vec<u8>,
}

impl BusMessage for HebEvent {
    fn topic(&self) -> &[u8] {
        self.header.category.as_bytes()
    }

    fn to_frames(&self) -> swissarmyhammer_leader_election::Result<Vec<Vec<u8>>> {
        let header_json = serde_json::to_vec(&self.header).map_err(ElectionError::Serialization)?;
        Ok(vec![header_json, self.body.clone()])
    }

    fn from_frames(
        _topic: &[u8],
        frames: &[Vec<u8>],
    ) -> swissarmyhammer_leader_election::Result<Self> {
        if frames.len() < 2 {
            return Err(ElectionError::Message(
                "HebEvent requires at least 2 frames (header + body)".to_string(),
            ));
        }
        let header: EventHeader =
            serde_json::from_slice(&frames[0]).map_err(ElectionError::Serialization)?;
        Ok(HebEvent {
            header,
            body: frames[1].clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::EventCategory;

    #[test]
    fn test_heb_event_roundtrip() {
        let header = EventHeader::new(
            "sess-1",
            "/workspace",
            EventCategory::Hook,
            "pre_tool_use",
            "avp-hook",
        );
        let event = HebEvent {
            header,
            body: b"test payload".to_vec(),
        };

        assert_eq!(event.topic(), b"hook");

        let frames = event.to_frames().unwrap();
        assert_eq!(frames.len(), 2);

        let restored = HebEvent::from_frames(b"hook", &frames).unwrap();
        assert_eq!(restored.header.session_id, "sess-1");
        assert_eq!(restored.body, b"test payload");
    }
}
