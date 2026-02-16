use std::path::Path;

#[derive(Default)]
pub struct ExifInfo {
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub focal_length: Option<String>,
    pub aperture: Option<String>,
    pub exposure: Option<String>,
    pub iso: Option<String>,
}

impl ExifInfo {
    pub fn summary(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();

        if let Some(camera) = &self.camera {
            parts.push(camera);
        }
        if let Some(lens) = &self.lens {
            parts.push(lens);
        }

        let mut settings = Vec::new();
        if let Some(fl) = &self.focal_length {
            settings.push(fl.to_string());
        }
        if let Some(ap) = &self.aperture {
            settings.push(format!("\u{192}/{}", ap));
        }
        if let Some(ex) = &self.exposure {
            settings.push(format!("{}s", ex));
        }
        if let Some(iso) = &self.iso {
            settings.push(format!("ISO {}", iso));
        }

        let settings_str = settings.join("  ");
        if !settings_str.is_empty() {
            parts.push(&settings_str);
        }

        parts.join(" · ")
    }
}

pub fn read_exif(path: &Path) -> Option<exif::Exif> {
    let file = std::fs::File::open(path).ok()?;
    let mut bufreader = std::io::BufReader::new(file);
    exif::Reader::new()
        .read_from_container(&mut bufreader)
        .ok()
}

fn clean_exif_value(raw: &str) -> Option<String> {
    let val = raw.trim_matches('"');
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

pub fn exif_field(exif: &exif::Exif, tag: exif::Tag) -> Option<String> {
    let field = exif.get_field(tag, exif::In::PRIMARY)?;
    clean_exif_value(&field.display_value().to_string())
}

fn camera_name(make: Option<String>, model: Option<String>) -> Option<String> {
    match (make, model) {
        (Some(make), Some(model)) => {
            if model.starts_with(&make) {
                Some(model)
            } else {
                Some(format!("{} {}", make, model))
            }
        }
        (None, Some(model)) => Some(model),
        (Some(make), None) => Some(make),
        (None, None) => None,
    }
}

pub fn read_exif_info(path: &Path) -> ExifInfo {
    let Some(exif) = read_exif(path) else {
        return ExifInfo::default();
    };

    let camera = camera_name(
        exif_field(&exif, exif::Tag::Make),
        exif_field(&exif, exif::Tag::Model),
    );

    ExifInfo {
        camera,
        lens: exif_field(&exif, exif::Tag::LensModel),
        focal_length: exif_field(&exif, exif::Tag::FocalLength).map(|fl| format!("{} mm", fl)),
        aperture: exif_field(&exif, exif::Tag::FNumber),
        exposure: exif_field(&exif, exif::Tag::ExposureTime),
        iso: exif_field(&exif, exif::Tag::PhotographicSensitivity),
    }
}

pub fn read_exif_date(path: &Path) -> Option<String> {
    let exif = read_exif(path)?;
    exif_field(&exif, exif::Tag::DateTimeOriginal)
}

pub fn format_year_month(datetime_str: &str) -> String {
    // EXIF date format: "2024-06-15 12:00:00" or "2024:06:15 12:00:00"
    let parts: Vec<&str> = datetime_str.split(['-', ':', ' ']).collect();
    if parts.len() >= 2 {
        let year = parts[0];
        let month_num: u32 = parts[1].parse().unwrap_or(0);
        let month_name = match month_num {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => return datetime_str.to_string(),
        };
        format!("{} {}", month_name, year)
    } else {
        datetime_str.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/DSCF0199.jpg")
    }

    #[test]
    fn read_exif_info_from_jpeg() {
        let info = read_exif_info(&fixture_path());

        assert_eq!(info.camera.as_deref(), Some("FUJIFILM X-T5"));
        assert_eq!(
            info.lens.as_deref(),
            Some("Fujifilm Fujinon XF18mmF1.4 R LM WR")
        );
        assert_eq!(info.focal_length.as_deref(), Some("18 mm"));
        assert_eq!(info.aperture.as_deref(), Some("5.6"));
        assert_eq!(info.exposure.as_deref(), Some("1/280"));
        assert_eq!(info.iso.as_deref(), Some("125"));
    }

    #[test]
    fn read_exif_date_from_jpeg() {
        let date = read_exif_date(&fixture_path());
        assert_eq!(date.as_deref(), Some("2026-02-01 15:01:06"));
    }

    #[test]
    fn read_exif_info_missing_file() {
        let info = read_exif_info(Path::new("/nonexistent/photo.jpg"));

        assert!(info.camera.is_none());
        assert!(info.lens.is_none());
        assert!(info.focal_length.is_none());
        assert!(info.aperture.is_none());
        assert!(info.exposure.is_none());
        assert!(info.iso.is_none());
    }

    #[test]
    fn read_exif_date_missing_file() {
        assert!(read_exif_date(Path::new("/nonexistent/photo.jpg")).is_none());
    }

    #[test]
    fn summary_all_fields() {
        let info = read_exif_info(&fixture_path());
        let summary = info.summary();

        assert!(summary.contains("FUJIFILM X-T5"));
        assert!(summary.contains("XF18mmF1.4"));
        assert!(summary.contains("18 mm"));
        assert!(summary.contains("\u{192}/5.6"));
        assert!(summary.contains("1/280s"));
        assert!(summary.contains("ISO 125"));
    }

    #[test]
    fn summary_empty() {
        let info = ExifInfo::default();
        assert_eq!(info.summary(), "");
    }

    #[test]
    fn summary_camera_only() {
        let info = ExifInfo {
            camera: Some("FUJIFILM X-T5".to_string()),
            ..Default::default()
        };
        assert_eq!(info.summary(), "FUJIFILM X-T5");
    }

    #[test]
    fn summary_settings_only() {
        let info = ExifInfo {
            aperture: Some("2.8".to_string()),
            iso: Some("400".to_string()),
            ..Default::default()
        };
        assert_eq!(info.summary(), "\u{192}/2.8  ISO 400");
    }

    #[test]
    fn camera_deduplicates_make_in_model() {
        // When model already starts with make, don't repeat it.
        // This is what FUJIFILM does: Make="FUJIFILM", Model="X-T5"
        // so result should be "FUJIFILM X-T5", not "FUJIFILM FUJIFILM X-T5"
        let info = read_exif_info(&fixture_path());
        assert_eq!(info.camera.as_deref(), Some("FUJIFILM X-T5"));
    }

    #[test]
    fn camera_name_model_starts_with_make() {
        let result = camera_name(Some("Canon".into()), Some("Canon EOS R5".into()));
        assert_eq!(result.as_deref(), Some("Canon EOS R5"));
    }

    #[test]
    fn camera_name_make_and_model() {
        let result = camera_name(Some("FUJIFILM".into()), Some("X-T5".into()));
        assert_eq!(result.as_deref(), Some("FUJIFILM X-T5"));
    }

    #[test]
    fn camera_name_model_only() {
        let result = camera_name(None, Some("X-T5".into()));
        assert_eq!(result.as_deref(), Some("X-T5"));
    }

    #[test]
    fn camera_name_make_only() {
        let result = camera_name(Some("FUJIFILM".into()), None);
        assert_eq!(result.as_deref(), Some("FUJIFILM"));
    }

    #[test]
    fn camera_name_none() {
        assert!(camera_name(None, None).is_none());
    }

    #[test]
    fn format_year_month_colon_separated() {
        assert_eq!(format_year_month("2026:02:01 15:01:06"), "February 2026");
    }

    #[test]
    fn format_year_month_dash_separated() {
        assert_eq!(format_year_month("2024-06-15 12:00:00"), "June 2024");
    }

    #[test]
    fn format_year_month_all_months() {
        let expected = [
            ("2024:01:15 12:00:00", "January 2024"),
            ("2024:03:15 12:00:00", "March 2024"),
            ("2024:04:15 12:00:00", "April 2024"),
            ("2024:05:15 12:00:00", "May 2024"),
            ("2024:07:15 12:00:00", "July 2024"),
            ("2024:08:15 12:00:00", "August 2024"),
            ("2024:09:15 12:00:00", "September 2024"),
            ("2024:10:15 12:00:00", "October 2024"),
            ("2024:11:15 12:00:00", "November 2024"),
            ("2024:12:15 12:00:00", "December 2024"),
        ];
        for (input, output) in expected {
            assert_eq!(format_year_month(input), output);
        }
    }

    #[test]
    fn format_year_month_invalid() {
        assert_eq!(format_year_month("garbage"), "garbage");
    }

    #[test]
    fn format_year_month_invalid_month() {
        assert_eq!(format_year_month("2024:13:01 00:00:00"), "2024:13:01 00:00:00");
    }

    #[test]
    fn clean_exif_value_normal() {
        assert_eq!(clean_exif_value("hello"), Some("hello".into()));
    }

    #[test]
    fn clean_exif_value_quoted() {
        assert_eq!(clean_exif_value("\"hello\""), Some("hello".into()));
    }

    #[test]
    fn clean_exif_value_empty() {
        assert!(clean_exif_value("").is_none());
    }

    #[test]
    fn clean_exif_value_only_quotes() {
        assert!(clean_exif_value("\"\"").is_none());
    }

    #[test]
    fn exif_field_missing_tag() {
        let exif = read_exif(&fixture_path()).unwrap();
        assert!(exif_field(&exif, exif::Tag::GPSLatitude).is_none());
    }
}
