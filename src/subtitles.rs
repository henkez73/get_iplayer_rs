use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SubtitleCue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub color: Option<String>,
}

pub struct SubtitleConverter;

impl SubtitleConverter {
    /// Parses TTML XML and converts to SRT string
    pub fn ttml_to_srt(
        ttml_xml: &str,
        mono: bool,
        offset_ms: i64,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut reader = Reader::from_str(ttml_xml);
        reader.config_mut().trim_text(true);

        let mut style_colors: HashMap<String, String> = HashMap::new();
        let mut cues: Vec<SubtitleCue> = Vec::new();
        let mut buf = Vec::new();

        let mut in_p = false;
        let mut current_begin = 0u64;
        let mut current_end = 0u64;
        let mut current_text = String::new();
        let mut current_style = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let name = e.name();
                    if name.as_ref() == "style" {
                        let mut style_id = String::new();
                        let mut color = String::new();
                        for attr in e.attributes().flatten() {
                            let k = attr.key.as_ref();
                            let v = attr.value.as_ref();
                            if k == "xml:id" || k == "id" {
                                style_id = v.to_string();
                            } else if k.ends_with("color") {
                                color = v.to_string();
                            }
                        }
                        if !style_id.is_empty() && !color.is_empty() {
                            style_colors.insert(style_id, color);
                        }
                    } else if name.as_ref() == "p" {
                        in_p = true;
                        current_begin = 0;
                        current_end = 0;
                        current_text.clear();
                        current_style = None;

                        for attr in e.attributes().flatten() {
                            let k = attr.key.as_ref();
                            let v = attr.value.as_ref();
                            match k {
                                "begin" => current_begin = Self::parse_ttml_time(v),
                                "end" => current_end = Self::parse_ttml_time(v),
                                "dur" => {
                                    let dur = Self::parse_ttml_time(v);
                                    if current_end == 0 && dur > 0 {
                                        current_end = current_begin + dur;
                                    }
                                }
                                "style" => current_style = Some(v.to_string()),
                                _ => {}
                            }
                        }
                    } else if name.as_ref() == "br" && in_p {
                        current_text.push('\n');
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_p {
                        let text = htmlescape::decode_html(e.as_ref()).unwrap_or_else(|_| e.as_ref().to_string());
                        current_text.push_str(&text);
                    }
                }
                Ok(Event::End(ref e)) => {
                    if e.name().as_ref() == "p" {
                        in_p = false;
                        let text = current_text.trim().to_string();
                        if !text.is_empty() && current_end > current_begin {
                            let mut start_ms = current_begin;
                            let mut end_ms = current_end;
                            if offset_ms > 0 {
                                start_ms = start_ms.saturating_add(offset_ms as u64);
                                end_ms = end_ms.saturating_add(offset_ms as u64);
                            } else if offset_ms < 0 {
                                let abs = (-offset_ms) as u64;
                                start_ms = start_ms.saturating_sub(abs);
                                end_ms = end_ms.saturating_sub(abs);
                            }

                            let color = current_style.as_ref().and_then(|s| style_colors.get(s)).cloned();
                            cues.push(SubtitleCue {
                                start_ms,
                                end_ms,
                                text,
                                color,
                            });
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        let mut srt = String::new();
        for (i, cue) in cues.iter().enumerate() {
            srt.push_str(&(i + 1).to_string());
            srt.push('\n');
            srt.push_str(&Self::format_srt_time(cue.start_ms));
            srt.push_str(" --> ");
            srt.push_str(&Self::format_srt_time(cue.end_ms));
            srt.push('\n');

            let formatted_text = if mono {
                let lines: Vec<&str> = cue.text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
                lines.iter().map(|l| format!("- {}", l)).collect::<Vec<_>>().join("\n")
            } else if let Some(ref col) = cue.color {
                format!("<font color=\"{}\">{}</font>", col, cue.text)
            } else {
                cue.text.clone()
            };

            srt.push_str(&formatted_text);
            srt.push_str("\n\n");
        }

        Ok(srt)
    }

    /// Parses TTML time string like "00:01:23.456" or "83.456s" or "123456ms" into milliseconds
    pub fn parse_ttml_time(time_str: &str) -> u64 {
        let s = time_str.trim();
        if let Some(num_s) = s.strip_suffix("ms") {
            return num_s.parse::<u64>().unwrap_or(0);
        }
        if let Some(num_s) = s.strip_suffix('s') {
            if let Ok(sec) = num_s.parse::<f64>() {
                return (sec * 1000.0) as u64;
            }
        }

        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 3 {
            let h: u64 = parts[0].parse().unwrap_or(0);
            let m: u64 = parts[1].parse().unwrap_or(0);
            let sec_parts: Vec<&str> = parts[2].split('.').collect();
            let s: u64 = sec_parts[0].parse().unwrap_or(0);
            let ms: u64 = if sec_parts.len() > 1 {
                let ms_str = format!("{:0<3}", sec_parts[1]);
                ms_str[..3.min(ms_str.len())].parse().unwrap_or(0)
            } else {
                0
            };
            return h * 3600_000 + m * 60_000 + s * 1000 + ms;
        }

        0
    }

    /// Formats milliseconds into SRT time format: HH:MM:SS,mmm
    pub fn format_srt_time(ms: u64) -> String {
        let total_secs = ms / 1000;
        let millis = ms % 1000;
        let h = total_secs / 3600;
        let m = (total_secs % 3600) / 60;
        let s = total_secs % 60;
        format!("{:02}:{:02}:{:02},{:03}", h, m, s, millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttml_to_srt_conversion() {
        let ttml = r##"<?xml version="1.0" encoding="utf-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <head>
    <styling>
      <style xml:id="s1" tts:color="#ffff00"/>
    </styling>
  </head>
  <body>
    <div>
      <p begin="00:01:23.450" end="00:01:26.780" style="s1">Hello world!</p>
      <p begin="00:01:27.000" end="00:01:30.000">Goodbye world!</p>
    </div>
  </body>
</tt>"##;

        let srt = SubtitleConverter::ttml_to_srt(ttml, false, 0).unwrap();
        assert!(srt.contains("1\n00:01:23,450 --> 00:01:26,780\n<font color=\"#ffff00\">Hello world!</font>"));
        assert!(srt.contains("2\n00:01:27,000 --> 00:01:30,000\nGoodbye world!"));

        let srt_mono = SubtitleConverter::ttml_to_srt(ttml, true, 0).unwrap();
        assert!(srt_mono.contains("- Hello world!"));
    }
}
