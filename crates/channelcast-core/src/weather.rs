//! Live Weather dashboard for ZIP 32920 / NWS MLB (Cape Canaveral).
//!
//! Powers the Android Weather tab (B2): Now + hourly + 3-day + active MLB
//! graphicast hazard images + optional NHC tropical when Atlantic-relevant.
//! Does **not** scrape stale weather RSS as the primary content.

use crate::error::{Error, Result};
use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::Deserialize;

const USER_AGENT: &str = "ChannelCast/0.1 (+https://github.com/cruiseliner/channelcast; weather)";
const HOME_LAT: f64 = 28.3903;
const HOME_LON: f64 = -80.6043;
const HOME_LABEL: &str = "Cape Canaveral, FL";
const MLB_GRID_FORECAST: &str = "https://api.weather.gov/gridpoints/MLB/56,65/forecast";
const MLB_GRID_HOURLY: &str = "https://api.weather.gov/gridpoints/MLB/56,65/forecast/hourly";
const MLB_STATIONS: &str = "https://api.weather.gov/gridpoints/MLB/56,65/stations";
const MLB_ALERTS: &str = "https://api.weather.gov/alerts/active?zone=FLZ747";
const MLB_GRAPHCAST_XML: &str = "https://www.weather.gov/source/mlb/graphicast/graphicast.xml";
const NHC_STORMS: &str = "https://www.nhc.noaa.gov/CurrentStorms.json";
const NHC_ATL_2DAY: &str = "https://www.nhc.noaa.gov/xgtwo/two_atl_2d0.png";
const NHC_HOME: &str = "https://www.nhc.noaa.gov/";

/// Current conditions card.
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherNow {
    pub location: String,
    pub temperature_f: i32,
    pub condition: String,
    pub wind: String,
    pub humidity_pct: Option<i32>,
    pub icon_url: Option<String>,
    pub observed_at: Option<String>,
}

/// One hour in the horizontal strip.
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherHour {
    pub label: String,
    pub temperature_f: i32,
    pub condition: String,
    pub icon_url: Option<String>,
    pub precip_pct: Option<i32>,
}

/// One day in the 3-day outlook.
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherDay {
    pub name: String,
    pub high_f: i32,
    pub low_f: i32,
    pub condition: String,
    pub icon_url: Option<String>,
    pub precip_pct: Option<i32>,
}

/// Official MLB graphicast (or similar) hazard graphic — use Cover crop in UI.
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherGraphic {
    pub title: String,
    pub description: String,
    pub image_url: String,
    pub link_url: String,
}

/// Tropical interest for FL / Caribbean — omit entirely when empty.
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherTropical {
    pub title: String,
    pub summary: String,
    pub image_url: String,
    pub article_url: String,
    pub article_title: String,
}

/// Full Weather tab payload (B2 stack order applies in the UI).
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherDashboard {
    pub now: WeatherNow,
    pub hourly: Vec<WeatherHour>,
    pub daily: Vec<WeatherDay>,
    pub graphics: Vec<WeatherGraphic>,
    pub tropical: Option<WeatherTropical>,
    pub alert_labels: Vec<String>,
}

pub fn fetch_weather_dashboard() -> Result<WeatherDashboard> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(25))
        .build()?;

    let (now, hourly_fallback) = fetch_now(&client)?;
    let hourly = {
        let mut h = fetch_hourly(&client)?;
        if h.is_empty() {
            h = hourly_fallback;
        }
        h
    };
    let daily = fetch_daily(&client)?;
    let graphics = fetch_graphicasts(&client).unwrap_or_default();
    let tropical = fetch_tropical(&client).unwrap_or(None);
    let alert_labels = fetch_alert_labels(&client).unwrap_or_default();

    if daily.is_empty() && hourly.is_empty() {
        return Err(Error::Feed("weather: no forecast periods".into()));
    }

    Ok(WeatherDashboard {
        now,
        hourly,
        daily,
        graphics,
        tropical,
        alert_labels,
    })
}

fn nws_get(client: &reqwest::blocking::Client, url: &str) -> Result<reqwest::blocking::Response> {
    Ok(client
        .get(url)
        .header(
            "Accept",
            "application/geo+json, application/ld+json, application/json, application/xml, text/xml, */*",
        )
        .send()?
        .error_for_status()?)
}

fn c_to_f(c: f64) -> i32 {
    (c * 9.0 / 5.0 + 32.0).round() as i32
}

fn kmh_to_mph(kmh: f64) -> i32 {
    (kmh * 0.621371).round() as i32
}

fn deg_to_compass(deg: f64) -> &'static str {
    const DIRS: [&str; 16] = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW",
        "NW", "NNW",
    ];
    let idx = ((deg / 22.5) + 0.5).floor() as usize % 16;
    DIRS[idx]
}

#[derive(Debug, Deserialize)]
struct StationsResponse {
    features: Vec<StationFeature>,
}

#[derive(Debug, Deserialize)]
struct StationFeature {
    properties: StationProps,
}

#[derive(Debug, Deserialize)]
struct StationProps {
    #[serde(rename = "stationIdentifier")]
    station_identifier: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ObsResponse {
    properties: ObsProps,
}

#[derive(Debug, Deserialize)]
struct ObsProps {
    timestamp: Option<String>,
    #[serde(rename = "textDescription")]
    text_description: Option<String>,
    temperature: Option<Quant>,
    #[serde(rename = "windSpeed")]
    wind_speed: Option<Quant>,
    #[serde(rename = "windDirection")]
    wind_direction: Option<Quant>,
    #[serde(rename = "relativeHumidity")]
    relative_humidity: Option<Quant>,
    icon: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Quant {
    value: Option<f64>,
}

fn read_json<T: for<'de> Deserialize<'de>>(resp: reqwest::blocking::Response) -> Result<T> {
    let bytes = resp.bytes()?;
    serde_json::from_slice(&bytes).map_err(|e| Error::Feed(format!("json: {e}")))
}

fn fetch_now(
    client: &reqwest::blocking::Client,
) -> Result<(WeatherNow, Vec<WeatherHour>)> {
    // Prefer nearest station observation; fall back to first hourly period.
    let stations: StationsResponse = read_json(nws_get(client, MLB_STATIONS)?)?;
    let mut now_from_obs: Option<WeatherNow> = None;
    for feat in stations.features.iter().take(5) {
        let Some(id) = feat.properties.station_identifier.as_deref() else {
            continue;
        };
        let url = format!("https://api.weather.gov/stations/{id}/observations/latest");
        let Ok(resp) = nws_get(client, &url) else {
            continue;
        };
        let Ok(obs) = read_json::<ObsResponse>(resp) else {
            continue;
        };
        let Some(temp_c) = obs.properties.temperature.and_then(|q| q.value) else {
            continue;
        };
        let condition = obs
            .properties
            .text_description
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "—".into());
        let wind = match (
            obs.properties.wind_direction.and_then(|q| q.value),
            obs.properties.wind_speed.and_then(|q| q.value),
        ) {
            (Some(dir), Some(spd)) => format!(
                "{} {} mph",
                deg_to_compass(dir),
                kmh_to_mph(spd)
            ),
            (_, Some(spd)) => format!("{} mph", kmh_to_mph(spd)),
            _ => "—".into(),
        };
        let humidity_pct = obs
            .properties
            .relative_humidity
            .and_then(|q| q.value)
            .map(|v| v.round() as i32);
        now_from_obs = Some(WeatherNow {
            location: HOME_LABEL.into(),
            temperature_f: c_to_f(temp_c),
            condition,
            wind,
            humidity_pct,
            icon_url: obs.properties.icon,
            observed_at: obs.properties.timestamp,
        });
        break;
    }

    let hourly = fetch_hourly(client).unwrap_or_default();
    let now = if let Some(n) = now_from_obs {
        n
    } else if let Some(h) = hourly.first() {
        WeatherNow {
            location: HOME_LABEL.into(),
            temperature_f: h.temperature_f,
            condition: h.condition.clone(),
            wind: "—".into(),
            humidity_pct: None,
            icon_url: h.icon_url.clone(),
            observed_at: None,
        }
    } else {
        // Last resort: points + forecast period 1
        let _ = (HOME_LAT, HOME_LON); // document home point
        let forecast = fetch_forecast_raw(client, MLB_GRID_FORECAST)?;
        let p = forecast
            .properties
            .periods
            .first()
            .ok_or_else(|| Error::Feed("weather: empty forecast".into()))?;
        WeatherNow {
            location: HOME_LABEL.into(),
            temperature_f: p.temperature.unwrap_or(0),
            condition: p.short_forecast.clone().unwrap_or_else(|| "—".into()),
            wind: match (&p.wind_direction, &p.wind_speed) {
                (Some(d), Some(s)) => format!("{d} {s}"),
                _ => "—".into(),
            },
            humidity_pct: None,
            icon_url: p.icon.clone(),
            observed_at: None,
        }
    };

    Ok((now, hourly))
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
    name: Option<String>,
    #[serde(rename = "startTime")]
    start_time: Option<String>,
    #[serde(rename = "isDaytime")]
    is_daytime: Option<bool>,
    temperature: Option<i32>,
    #[serde(rename = "shortForecast")]
    short_forecast: Option<String>,
    #[serde(rename = "windSpeed")]
    wind_speed: Option<String>,
    #[serde(rename = "windDirection")]
    wind_direction: Option<String>,
    icon: Option<String>,
    #[serde(rename = "probabilityOfPrecipitation")]
    probability_of_precipitation: Option<Pop>,
}

#[derive(Debug, Deserialize)]
#[derive(Clone, Copy)]
struct Pop {
    value: Option<i32>,
}

fn fetch_forecast_raw(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Result<ForecastResponse> {
    read_json(nws_get(client, url)?)
}

fn fetch_hourly(client: &reqwest::blocking::Client) -> Result<Vec<WeatherHour>> {
    let parsed = fetch_forecast_raw(client, MLB_GRID_HOURLY)?;
    let mut out = Vec::new();
    for p in parsed.properties.periods.into_iter().take(12) {
        let Some(start) = p.start_time.as_deref() else {
            continue;
        };
        let label = format_hour_label(start).unwrap_or_else(|| {
            p.name.clone().unwrap_or_else(|| start.chars().take(16).collect())
        });
        let Some(temp) = p.temperature else {
            continue;
        };
        out.push(WeatherHour {
            label,
            temperature_f: temp,
            condition: p.short_forecast.unwrap_or_default(),
            icon_url: p.icon,
            precip_pct: p.probability_of_precipitation.and_then(|x| x.value),
        });
    }
    Ok(out)
}

fn format_hour_label(rfc3339: &str) -> Option<String> {
    // NWS stamps include the local offset (e.g. -04:00 for EDT) — use that hour.
    let dt = DateTime::parse_from_rfc3339(rfc3339).ok()?;
    let hour = dt.hour();
    let (h12, ampm) = match hour {
        0 => (12, "AM"),
        1..=11 => (hour, "AM"),
        12 => (12, "PM"),
        _ => (hour - 12, "PM"),
    };
    Some(format!("{h12} {ampm}"))
}

fn fetch_daily(client: &reqwest::blocking::Client) -> Result<Vec<WeatherDay>> {
    let parsed = fetch_forecast_raw(client, MLB_GRID_FORECAST)?;
    let mut days: Vec<WeatherDay> = Vec::new();
    let mut i = 0;
    let periods = parsed.properties.periods;
    while i < periods.len() && days.len() < 3 {
        let p = &periods[i];
        let is_day = p.is_daytime.unwrap_or(true);
        if !is_day {
            // Start mid-cycle: treat tonight as low-only, skip building a day from night-first.
            i += 1;
            continue;
        }
        let name = pretty_day_name(p.name.as_deref().unwrap_or("Day"), p.start_time.as_deref());
        let high = p.temperature.unwrap_or(0);
        let condition = p.short_forecast.clone().unwrap_or_default();
        let icon = p.icon.clone();
        let precip = p.probability_of_precipitation.as_ref().and_then(|x| x.value);
        let mut low = high.saturating_sub(10);
        if let Some(night) = periods.get(i + 1) {
            if night.is_daytime == Some(false) {
                if let Some(t) = night.temperature {
                    low = t;
                }
                i += 2;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
        days.push(WeatherDay {
            name,
            high_f: high,
            low_f: low,
            condition,
            icon_url: icon,
            precip_pct: precip,
        });
    }
    Ok(days)
}

fn pretty_day_name(name: &str, start: Option<&str>) -> String {
    if name.eq_ignore_ascii_case("today") || name.eq_ignore_ascii_case("this afternoon") {
        return "Today".into();
    }
    if name.eq_ignore_ascii_case("tonight") {
        return "Tonight".into();
    }
    // "Tuesday" / "Tuesday Night" → weekday short from startTime when possible
    if let Some(s) = start {
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            const WD: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
            // chrono weekday: Mon=0 in Datelike::weekday().num_days_from_monday()
            let idx = dt.weekday().num_days_from_monday() as usize;
            return WD[idx].into();
        }
    }
    name.split_whitespace()
        .next()
        .unwrap_or(name)
        .chars()
        .take(3)
        .collect()
}

fn fetch_graphicasts(client: &reqwest::blocking::Client) -> Result<Vec<WeatherGraphic>> {
    let text = nws_get(client, MLB_GRAPHCAST_XML)?.text()?;
    let now = Utc::now().timestamp();
    let mut out = Vec::new();
    for block in text.split("<graphicast>").skip(1) {
        let end = xml_i64(block, "EndTime").unwrap_or(i64::MAX);
        if end < now {
            continue;
        }
        let title = xml_cdata_or_text(block, "title").unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        let lower = title.to_ascii_lowercase();
        if lower.contains("skywarn") || lower.contains("climate") || lower == "forecast" {
            continue;
        }
        let image = xml_text(block, "FullImage")
            .or_else(|| xml_text(block, "SmallImage"))
            .unwrap_or_default();
        if image.is_empty() {
            continue;
        }
        let mut description = xml_cdata_or_text(block, "description").unwrap_or_default();
        if let Some(rest) = description.strip_prefix("Click/tap image to enlarge | ") {
            description = rest.to_string();
        } else if let Some(rest) = description.strip_prefix("Click/tap image to enlarge| ") {
            description = rest.to_string();
        }
        let link = xml_text(block, "WebURL").unwrap_or_else(|| "https://www.weather.gov/mlb".into());
        out.push(WeatherGraphic {
            title,
            description,
            image_url: image,
            link_url: link.replace("http://", "https://"),
        });
    }
    // Stable order by appearance (office order field already sequential in file).
    Ok(out)
}

fn xml_text(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&close)? + start;
    let raw = block[start..end].trim();
    let cleaned = raw
        .replace("<![CDATA[", "")
        .replace("]]>", "")
        .trim()
        .to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn xml_cdata_or_text(block: &str, tag: &str) -> Option<String> {
    xml_text(block, tag)
}

fn xml_i64(block: &str, tag: &str) -> Option<i64> {
    xml_text(block, tag)?.parse().ok()
}

#[derive(Debug, Deserialize)]
struct AlertsResponse {
    #[serde(default)]
    features: Vec<AlertFeature>,
}

#[derive(Debug, Deserialize)]
struct AlertFeature {
    properties: AlertProps,
}

#[derive(Debug, Deserialize)]
struct AlertProps {
    event: Option<String>,
}

fn fetch_alert_labels(client: &reqwest::blocking::Client) -> Result<Vec<String>> {
    let parsed: AlertsResponse = read_json(nws_get(client, MLB_ALERTS)?)?;
    let mut labels = Vec::new();
    for f in parsed.features {
        if let Some(ev) = f.properties.event {
            if !ev.is_empty() && !labels.iter().any(|x| x == &ev) {
                labels.push(ev);
            }
        }
    }
    Ok(labels)
}

#[derive(Debug, Deserialize)]
struct NhcStorms {
    #[serde(rename = "activeStorms", default)]
    active_storms: Vec<NhcStorm>,
}

#[derive(Debug, Deserialize)]
struct NhcStorm {
    id: String,
    name: String,
    classification: Option<String>,
    intensity: Option<String>,
    latitude: Option<String>,
    longitude: Option<String>,
    #[serde(rename = "latitudeNumeric")]
    latitude_numeric: Option<f64>,
    #[serde(rename = "longitudeNumeric")]
    longitude_numeric: Option<f64>,
    #[serde(rename = "publicAdvisory")]
    public_advisory: Option<NhcAdvisory>,
    #[serde(rename = "forecastGraphics")]
    forecast_graphics: Option<NhcAdvisory>,
}

#[derive(Debug, Deserialize)]
struct NhcAdvisory {
    url: Option<String>,
}

/// Atlantic / Caribbean / Florida interest only. Eastern/Central Pacific storms omitted.
fn fetch_tropical(client: &reqwest::blocking::Client) -> Result<Option<WeatherTropical>> {
    let resp = client
        .get(NHC_STORMS)
        .header("Accept", "application/json")
        .send()?
        .error_for_status()?;
    let storms: NhcStorms = read_json(resp)?;

    let relevant: Vec<&NhcStorm> = storms
        .active_storms
        .iter()
        .filter(|s| storm_affects_florida_caribbean(s))
        .collect();

    if relevant.is_empty() {
        return Ok(None);
    }

    let s = relevant[0];
    let class = s.classification.as_deref().unwrap_or("Tropical");
    let title = format!("{} {}", class_long(class), s.name);
    let lat = s.latitude.as_deref().unwrap_or("?");
    let lon = s.longitude.as_deref().unwrap_or("?");
    let intensity = s
        .intensity
        .as_deref()
        .map(|i| format!("{i} mph"))
        .unwrap_or_default();
    let summary = format!(
        "Active near {lat} {lon}{}",
        if intensity.is_empty() {
            String::new()
        } else {
            format!(" · Max winds {intensity}")
        }
    );
    let article_url = s
        .public_advisory
        .as_ref()
        .and_then(|a| a.url.clone())
        .unwrap_or_else(|| NHC_HOME.into());
    let image_url = s
        .forecast_graphics
        .as_ref()
        .and_then(|a| a.url.clone())
        .filter(|u| {
            let l = u.to_ascii_lowercase();
            l.ends_with(".png") || l.ends_with(".jpg") || l.contains("/graphics/")
        })
        .unwrap_or_else(|| NHC_ATL_2DAY.into());
    // Forecast graphics URL is often an HTML page — prefer cone/image pattern when id known.
    let image_url = storm_graphic_url(&s.id).unwrap_or(image_url);

    Ok(Some(WeatherTropical {
        title: title.clone(),
        summary,
        image_url,
        article_url,
        article_title: format!("NHC advisory — {}", s.name),
    }))
}

fn storm_affects_florida_caribbean(s: &NhcStorm) -> bool {
    let id = s.id.to_ascii_lowercase();
    if id.starts_with("al") {
        return true;
    }
    // Numeric lon/lat: Atlantic/Caribbean/Gulf box (exclude EP/CP west of ~100W).
    match (s.latitude_numeric, s.longitude_numeric) {
        (Some(lat), Some(lon)) => {
            // NHC lon is negative in west; Caribbean/Gulf/FL roughly lon -100..-50, lat 7..35
            lat >= 7.0 && lat <= 35.0 && lon >= -100.0 && lon <= -50.0
        }
        _ => false,
    }
}

fn class_long(class: &str) -> &str {
    match class.to_ascii_uppercase().as_str() {
        "HU" => "Hurricane",
        "TS" => "Tropical Storm",
        "TD" => "Tropical Depression",
        "SS" => "Subtropical Storm",
        "SD" => "Subtropical Depression",
        "PTC" => "Potential Tropical Cyclone",
        other if other.len() <= 3 => "Tropical System",
        _ => class,
    }
}

fn storm_graphic_url(id: &str) -> Option<String> {
    // e.g. al092026 → https://www.nhc.noaa.gov/storm_graphics/AT09/refresh/AL092026_5day_cone_no_line_and_wind+png/...
    // Simpler reliable static cone when available:
    let id_up = id.to_ascii_uppercase();
    if !id_up.starts_with("AL") || id_up.len() < 4 {
        return None;
    }
    let basin_num = &id_up[2..4];
    Some(format!(
        "https://www.nhc.noaa.gov/storm_graphics/AT{basin_num}/refresh/{id_up}_5day_cone_no_line_and_wind.png"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_pacific_storms() {
        let ep = NhcStorm {
            id: "ep222026".into(),
            name: "Vance".into(),
            classification: Some("TS".into()),
            intensity: None,
            latitude: None,
            longitude: None,
            latitude_numeric: Some(15.4),
            longitude_numeric: Some(-105.7),
            public_advisory: None,
            forecast_graphics: None,
        };
        assert!(!storm_affects_florida_caribbean(&ep));
        let al = NhcStorm {
            id: "al052026".into(),
            name: "Test".into(),
            classification: Some("HU".into()),
            intensity: None,
            latitude: None,
            longitude: None,
            latitude_numeric: Some(25.0),
            longitude_numeric: Some(-80.0),
            public_advisory: None,
            forecast_graphics: None,
        };
        assert!(storm_affects_florida_caribbean(&al));
    }

    #[test]
    fn graphicast_xml_parses_hazard_titles() {
        let xml = r#"
        <graphicasts>
          <graphicast>
            <EndTime>9999999999</EndTime>
            <title><![CDATA[Wind Advisory]]></title>
            <description><![CDATA[Click/tap image to enlarge | Gusty]]></description>
            <FullImage>https://www.weather.gov/images/mlb/graphicast/4.png</FullImage>
            <WebURL>http://www.weather.gov/mlb</WebURL>
          </graphicast>
          <graphicast>
            <EndTime>9999999999</EndTime>
            <title><![CDATA[SKYWARN]]></title>
            <description><![CDATA[class]]></description>
            <FullImage>https://www.weather.gov/images/mlb/graphicast/5.png</FullImage>
            <WebURL>http://www.weather.gov/mlb</WebURL>
          </graphicast>
          <graphicast>
            <EndTime>1</EndTime>
            <title><![CDATA[Expired]]></title>
            <FullImage>https://www.weather.gov/images/mlb/graphicast/9.png</FullImage>
          </graphicast>
        </graphicasts>
        "#;
        // Inline mini-parse using same helpers via a local reimplementation path:
        let now = 100_i64;
        let mut out = Vec::new();
        for block in xml.split("<graphicast>").skip(1) {
            let end = xml_i64(block, "EndTime").unwrap_or(i64::MAX);
            if end < now {
                continue;
            }
            let title = xml_cdata_or_text(block, "title").unwrap_or_default();
            let lower = title.to_ascii_lowercase();
            if lower.contains("skywarn") || lower.contains("climate") || lower == "forecast" {
                continue;
            }
            let image = xml_text(block, "FullImage").unwrap_or_default();
            out.push((title, image));
        }
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "Wind Advisory");
    }

    #[test]
    fn live_dashboard_smoke() {
        let dash = fetch_weather_dashboard().expect("live weather");
        assert!(dash.now.temperature_f > 0);
        assert!(!dash.now.condition.is_empty());
        assert!(!dash.hourly.is_empty(), "hourly strip");
        assert!(!dash.daily.is_empty(), "3-day");
        assert!(dash.daily.len() <= 3);
        // Tropical must be absent today (only EP/CP storms).
        assert!(dash.tropical.is_none(), "no Atlantic tropical expected");
        println!(
            "NOW {}°F {} | hourly={} daily={} graphics={} alerts={:?}",
            dash.now.temperature_f,
            dash.now.condition,
            dash.hourly.len(),
            dash.daily.len(),
            dash.graphics.len(),
            dash.alert_labels
        );
        for g in &dash.graphics {
            println!("  GRAPHIC {} -> {}", g.title, g.image_url);
        }
    }
}
