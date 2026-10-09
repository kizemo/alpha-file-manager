//! Event types emitted by the spike binary.

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "event")]
pub enum Event {
    ForegroundChanged {
        hwnd: u32,
        ts: String,
    },
    DialogDetected {
        hwnd: u32,
        app: String,
        ts: String,
    },
    DialogClosed {
        hwnd: u32,
        app: String,
        last_known_path: String,
        ts: String,
    },
    Read {
        hwnd: u32,
        app: String,
        path: String,
        strategy: String,
        ts: String,
    },
    Write {
        hwnd: u32,
        app: String,
        target: String,
        strategy: String,
        ts: String,
    },
    WriteFailed {
        hwnd: u32,
        app: String,
        target: String,
        reason: String,
        ts: String,
    },
    SpikeError {
        kind: String,
        message: String,
        ts: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_event_serializes() {
        let evt = Event::Read {
            hwnd: 12345,
            app: "chrome".into(),
            path: "C:\\Users\\foo".into(),
            strategy: "uia".into(),
            ts: "2026-09-27T12:34:56Z".into(),
        };
        let json = serde_json::to_string(&evt).unwrap();
        assert!(json.contains("\"event\":\"Read\""));
        assert!(json.contains("\"app\":\"chrome\""));
        assert!(json.contains("\"path\":\"C:\\\\Users\\\\foo\""));
    }

    #[test]
    fn write_event_serializes() {
        let evt = Event::Write {
            hwnd: 99,
            app: "edge".into(),
            target: "D:\\bar".into(),
            strategy: "uia".into(),
            ts: "2026-09-27T12:34:56Z".into(),
        };
        let json = serde_json::to_string(&evt).unwrap();
        assert!(json.contains("\"event\":\"Write\""));
        assert!(json.contains("\"strategy\":\"uia\""));
    }

    #[test]
    fn spike_error_event_serializes() {
        let evt = Event::SpikeError {
            kind: "startup".into(),
            message: "spike starting".into(),
            ts: "2026-09-27T12:34:56Z".into(),
        };
        let json = serde_json::to_string(&evt).unwrap();
        assert!(json.contains("\"event\":\"SpikeError\""));
        assert!(json.contains("\"kind\":\"startup\""));
    }
}