//! Minimal National Weather Service (api.weather.gov) forecast JSON → Article.
//! Alerts Atom feeds go through the normal RSS/Atom path.

use crate::error::{Error, Result};
use crate::model::Article;
use serde::Deserialize;

/// True for NWS gridpoint forecast JSON endpoints (not Atom/CAP).
pub fn is_nws_forecast_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("api.weather.gov/gridpoints/")
        && lower.contains("/forecast")
        && !lower.contains(".atom")
}

#[derive(Debug, Deserialize)]
struct ForecastResponse {
    properties: ForecastProperties,
}

#[derive(Debug, Deserialize)]
struct ForecastProperties {
    #[serde(default)]
    periods: Vec<ForecastPeriod>,
}

#[derive(Debug, Deserialize)]
struct ForecastPeriod {
    number: u32,
    name: String,
    #[serde(rename = "startTime")]
    start_time: Option<String>,
    #[serde(rename = "temperature")]
    temperature: Option<i32>,
    #[serde(rename = "temperatureUnit")]
    temperature_unit: Option<String>,
    #[serde(rename = "shortForecast")]
    short_forecast: Option<String>,
    #[serde(rename = "detailedForecast")]
    detailed_forecast: Option<String>,
    #[serde(rename = "windSpeed")]
    wind_speed: Option<String>,
    #[serde(rename = "windDirection")]
    wind_direction: Option<String>,
}

/// Parse NWS `/gridpoints/{wfo}/{x},{y}/forecast` GeoJSON into articles
/// (one per forecast period).
pub fn parse_nws_forecast_bytes(
    bytes: &[u8],
    source_url: &str,
    channel_id: &str,
) -> Result<Vec<Article>> {
    let parsed: ForecastResponse =
        serde_json::from_slice(bytes).map_err(|e| Error::Feed(format!("nws forecast json: {e}")))?;

    let source = format!("NWS {}", grid_label(source_url));
    let mut out = Vec::with_capacity(parsed.properties.periods.len());

    for period in parsed.properties.periods {
        let temp = match (period.temperature, period.temperature_unit.as_deref()) {
            (Some(t), Some(u)) => format!("{t}{u}"),
            (Some(t), None) => t.to_string(),
            _ => String::new(),
        };
        let short = period.short_forecast.unwrap_or_default();
        let title = if temp.is_empty() {
            format!("{}: {}", period.name, short)
        } else {
            format!("{}: {} {}", period.name, temp, short)
        };

        let mut summary_parts = Vec::new();
        if let Some(d) = period.detailed_forecast {
            if !d.is_empty() {
                summary_parts.push(d);
            }
        }
        if let (Some(ws), Some(wd)) = (period.wind_speed, period.wind_direction) {
            summary_parts.push(format!("Wind: {wd} {ws}"));
        }
        let summary = if summary_parts.is_empty() {
            None
        } else {
            Some(summary_parts.join(" "))
        };

        // Keep period start in the summary; leave published unset so global
        // newest-first ranking does not reverse the forecast sequence (alerts
        // with real timestamps still float above undated periods).
        let summary = match (period.start_time.as_deref(), summary) {
            (Some(start), Some(s)) => Some(format!("Starts {start}. {s}")),
            (Some(start), None) => Some(format!("Starts {start}.")),
            (None, s) => s,
        };

        // Unique URL per period so dedupe does not collapse the forecast.
        let url = format!("{source_url}#period-{}", period.number);

        out.push(Article {
            title,
            url,
            summary,
            published: None,
            source: source.clone(),
            channel_id: channel_id.to_string(),
        });
    }

    if out.is_empty() {
        return Err(Error::Feed("nws forecast: no periods".into()));
    }
    Ok(out)
}

fn grid_label(url: &str) -> String {
    // https://api.weather.gov/gridpoints/MLB/56,65/forecast → MLB 56,65
    if let Some(rest) = url.split("gridpoints/").nth(1) {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 {
            return format!("{} {}", parts[0], parts[1]);
        }
    }
    "forecast".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_forecast_urls() {
        assert!(is_nws_forecast_url(
            "https://api.weather.gov/gridpoints/MLB/56,65/forecast"
        ));
        assert!(is_nws_forecast_url(
            "https://api.weather.gov/gridpoints/MLB/56,65/forecast/hourly"
        ));
        assert!(!is_nws_forecast_url(
            "https://api.weather.gov/alerts/active.atom?zone=FLZ747"
        ));
        assert!(!is_nws_forecast_url("https://example.com/feed"));
    }
}
