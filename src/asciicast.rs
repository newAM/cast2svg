/// [asciicast] deserializer.
///
/// [asciicast]: https://github.com/asciinema/asciinema/tree/develop/doc
use std::collections::HashMap;

#[derive(serde::Deserialize, Debug)]
pub struct Theme {
    pub fg: String,
    pub bg: String,
    pub palette: String,
}

#[derive(serde::Deserialize, Debug)]
pub struct Header {
    pub version: u64,
    pub width: usize,
    pub height: usize,
    pub timestamp: Option<u128>,
    pub duration: Option<f64>,
    pub idle_time_limit: Option<f64>,
    pub command: Option<String>,
    pub title: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub theme: Option<Theme>,
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Copy, Clone)]
pub enum EventType {
    Input,
    Output,
}

impl<'de> serde::de::Deserialize<'de> for EventType {
    fn deserialize<D>(deserializer: D) -> Result<EventType, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        match <char>::deserialize(deserializer) {
            Ok('i') => Ok(EventType::Input),
            Ok('o') => Ok(EventType::Output),
            Ok(x) => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char(x),
                &"an 'i' or 'o'",
            )),
            Err(e) => Err(e),
        }
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct Event(f64, EventType, String);

impl Event {
    /// Create a new event.
    #[allow(dead_code)]
    pub fn new<T>(time: f64, etype: EventType, data: T) -> Event
    where
        T: ToString,
    {
        Event(time, etype, data.to_string())
    }

    /// Get the time of the event.
    pub fn time(&self) -> f64 {
        self.0
    }

    /// Get the event type.
    #[allow(dead_code)]
    pub fn event_type(&self) -> EventType {
        self.1
    }

    /// Get the event data.
    pub fn event_data(&self) -> &str {
        self.2.as_str()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn header() {
        let json_str: &str = r#"{
            "version": 2,
            "width": 80,
            "height": 24,
            "timestamp": 1504467315,
            "title": "Demo",
            "env": {"TERM": "xterm-256color", "SHELL": "/bin/zsh"}
        }"#;
        let header: Header = serde_json::from_str(json_str).unwrap();
        assert_eq!(header.version, 2);
        assert_eq!(header.width, 80);
        assert_eq!(header.height, 24);
        assert_eq!(header.timestamp, Some(1504467315));
        assert_eq!(header.title, Some("Demo".to_string()));
        let mut map: HashMap<String, String> = HashMap::new();
        map.insert("TERM".into(), "xterm-256color".into());
        map.insert("SHELL".into(), "/bin/zsh".into());
        assert_eq!(header.env, Some(map));
        assert!(header.theme.is_none());
    }

    #[test]
    fn event() {
        let json_str: &str = r##"[
            0.248848,
            "o",
            "\u001b[1;31mHello \u001b[32mWorld!\u001b[0m\n"
        ]"##;
        let event: Event = serde_json::from_str(json_str).unwrap();
        let expected: Event = Event::new(
            0.248848,
            EventType::Output,
            "\u{001b}[1;31mHello \u{001b}[32mWorld!\u{001b}[0m\n",
        );
        assert_eq!(event.event_type(), expected.event_type());
        assert_eq!(event.event_data(), expected.event_data());
        assert!((event.time() - expected.time()).abs() < 0.0000001);
    }
}
