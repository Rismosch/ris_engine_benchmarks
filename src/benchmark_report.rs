use core::f64;
use std::io::Read;
use std::path::Path;

use ris_error::prelude::*;

use crate::json::{JsonObject, JsonValue};

//==============================================================================
// Structs
//==============================================================================
#[derive(Debug, Clone)]
pub struct Benchmark {
    pub group_id: String,
    pub function_id: String,
    pub value_str: Option<String>,
    pub throughput: Option<String>,
    pub full_id: String,
    pub directory_name: String,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct ConfidenceInterval {
    pub confidence_level: f64,
    pub lower_bound: f64,
    pub upper_bound: f64,
}

#[derive(Debug, Clone)]
pub struct Estimate {
    pub confidence_interval: ConfidenceInterval,
    pub point_estimate: f64,
    pub standard_error: f64,
}

#[derive(Debug, Clone)]
pub struct Estimates {
    pub mean: Option<Estimate>,
    pub median: Option<Estimate>,
    pub median_abs_dev: Option<Estimate>,
    pub slope: Option<Estimate>,
    pub std_dev: Option<Estimate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Unit {
    ns,
}

#[derive(Debug, Clone)]
pub struct Raw {
    pub group: String,
    pub function: String,
    pub value: String,
    pub throughput_num: String,
    pub throughput_type: String,
    pub sample_measured_value: f64,
    pub unit: Unit,
    pub iteration_count: usize,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub benchmark: Benchmark,
    pub estimates: Estimates,
    pub raw: Vec<Raw>,
    pub sample: Samples,
    pub tukey: Tukey,
}

#[derive(Debug, Clone)]
pub struct Sample {
    pub iter: usize,
    pub time: f64,
}

#[derive(Debug, Clone)]
pub struct Samples {
    pub sampling_mode: String,
    pub data: Vec<Sample>,
}

#[derive(Debug, Clone)]
pub struct Tukey(pub Vec<f64>);

//==============================================================================
// Methods
//==============================================================================
impl Raw {
    pub fn iteration_measured_value(&self) -> f64 {
        self.sample_measured_value / self.iteration_count as f64
    }

    pub fn iteration_time(&self) -> std::time::Duration {
        let measured_iteration = self.iteration_measured_value();
        match self.unit {
            Unit::ns => std::time::Duration::from_nanos(measured_iteration.round() as u64),
        }
    }
}

impl Report {
    pub fn kde(&self, points: usize) -> RisResult<Vec<(f64, f64)>> {
        ris_error::assert!(!self.raw.is_empty())?;
        ris_error::assert!(points >= 2)?;

        let values = self
            .raw
            .iter()
            .map(|x| x.iteration_measured_value())
            .collect::<Vec<_>>();

        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;

        let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;

        let stddev = variance.sqrt();

        // Silverman's rule of thumb
        let bandwidth = if stddev > 0.0 {
            1.06 * stddev * n.powf(-0.2)
        } else {
            (mean.abs() * 1e-3).max(1.0)
        };

        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        // extend the grid beyond the observations to show KDE tails
        let lo = min - 4.0 * bandwidth;
        let hi = max + 4.0 * bandwidth;

        let inv = 1.0 / (bandwidth * (2.0 * std::f64::consts::PI).sqrt());

        Ok((0..points)
            .map(|i| {
                let x = lo + (hi - lo) * i as f64 / (points - 1) as f64;
                let density = values
                    .iter()
                    .map(|v| {
                        let z = (x - v) / bandwidth;
                        (-0.5 * z * z).exp()
                    })
                    .sum::<f64>()
                    * inv
                    / n;

                (x, density)
            })
            .collect())
    }
}

//==============================================================================
// Conversions
//==============================================================================
impl Benchmark {
    fn deserialize(value: JsonValue) -> RisResult<Self> {
        let JsonValue::Object(object) = value else {
            return ris_error::new_result!("json was not an object");
        };

        Ok(Self {
            group_id: object.get("group_id").ris_expect("member to exist")?,
            function_id: object.get("function_id").ris_expect("member to exist")?,
            value_str: object.get("value_str"),
            throughput: object.get("throughput"),
            full_id: object.get("full_id").ris_expect("member to exist")?,
            directory_name: object.get("directory_name").ris_expect("member to exist")?,
            title: object.get("title").ris_expect("member to exist")?,
        })
    }
}

impl Estimate {
    fn deserialize(value: Option<JsonValue>) -> RisResult<Option<Self>> {
        let object = match value {
            Some(JsonValue::Object(object)) => object,
            Some(JsonValue::Null) => return Ok(None),
            _ => return ris_error::new_result!("value was not an object"),
        };

        let confidence_interval_json = object.get::<JsonValue>("confidence_interval");
        let Some(JsonValue::Object(confidence_interval_object)) = confidence_interval_json else {
            return ris_error::new_result!("value was not an object");
        };

        let confidence_interval = ConfidenceInterval {
            confidence_level: confidence_interval_object
                .get("confidence_level")
                .ris_expect("member to exist")?,
            lower_bound: confidence_interval_object
                .get("lower_bound")
                .ris_expect("member to exist")?,
            upper_bound: confidence_interval_object
                .get("upper_bound")
                .ris_expect("member to exist")?,
        };

        let point_estimate = object.get("point_estimate").ris_expect("member to exist")?;
        let standard_error = object.get("standard_error").ris_expect("member to exist")?;

        Ok(Some(Self {
            confidence_interval,
            point_estimate,
            standard_error,
        }))
    }
}

impl Estimates {
    fn deserialize(value: JsonValue) -> RisResult<Self> {
        let JsonValue::Object(object) = value else {
            return ris_error::new_result!("json was not an object");
        };

        Ok(Self {
            mean: Estimate::deserialize(object.get("mean"))?,
            median: Estimate::deserialize(object.get("slope"))?,
            median_abs_dev: Estimate::deserialize(object.get("median_abs_dev"))?,
            slope: Estimate::deserialize(object.get("slope"))?,
            std_dev: Estimate::deserialize(object.get("std_dev"))?,
        })
    }
}

impl Raw {
    fn deserialize(value: String) -> RisResult<Vec<Self>> {
        let mut data = Vec::new();

        for line in value.lines().skip(1) {
            //let mut entries = Vec::new();

            //let mut splits = line.split("\"");
            //loop {
            //    let first = splits.next();
            //    let second = splits.next();

            //    match (first, second) {
            //        (Some(first), Some(second)) => {
            //            for entry in first.split(',') {
            //                if !entry.trim().is_empty() {
            //                    entries.push(entry);
            //                }
            //            }

            //            if !second.trim().is_empty() {
            //                entries.push(second);
            //            }
            //        },
            //        (Some(first), None) => {
            //            for entry in first.split(',') {
            //                if !entry.trim().is_empty() {
            //                    entries.push(entry);
            //                }
            //            }

            //            break;
            //        },
            //        _ => break,
            //    }
            //}

            //eprintln!("DEBUG: {:?}", entries);

            let mut naive_entries = line.split(',');
            let mut entries = Vec::new();

            loop {
                let Some(entry) = naive_entries.next() else {
                    break;
                };

                // if an entry is surrounded with `"`, it may contain `,`. 
                // we need to concat those
                let mut concat = vec![entry];
                if entry.trim_start().starts_with('\"') {
                    loop {
                        let Some(entry) = naive_entries.next() else {
                            eprintln!("WARNING: didn't found closing `\"`");
                            break;
                        };

                        concat.push(entry);

                        if entry.trim_end().ends_with('\"') {
                            break;
                        }
                    }
                }

                let mut entry = entry.to_string();

                for piece in concat.iter().skip(1) {
                    entry = format!("{},{}", entry, piece);
                }

                entries.push(entry);
            }

            let mut entries = entries.iter().map(|x| x.as_str());

            let group = entries.next().ris_expect("group to exist")?.to_string();
            let function = entries.next().ris_expect("function to exist")?.to_string();
            let value = entries.next().ris_expect("value to exist")?.to_string();
            let throughput_num = entries
                .next()
                .ris_expect("throughput_num to exist")?
                .to_string();
            let throughput_type = entries
                .next()
                .ris_expect("throughput_type to exist")?
                .to_string();
            let sample_measured_value = entries
                .next()
                .ris_expect("sample_measured_value to exist")?
                .parse()?;
            let unit = match entries.next().ris_expect("unit to exist")? {
                "ns" => Unit::ns,
                unit => return ris_error::new_result!("unkown unit {}", unit),
            };
            let iteration_count = entries
                .next()
                .ris_expect("iteration_count to exist")?
                .parse()?;

            data.push(Self {
                group,
                function,
                value,
                throughput_num,
                throughput_type,
                sample_measured_value,
                unit,
                iteration_count,
            });
        }

        Ok(data)
    }
}

impl Samples {
    fn deserialize(value: JsonValue) -> RisResult<Self> {
        let JsonValue::Object(object) = value else {
            return ris_error::new_result!("json was not an object");
        };

        let sampling_mode = object.get("sampling_mode").ris_expect("member to exist")?;

        let Some(JsonValue::Array(iters)) = object.get::<JsonValue>("iters") else {
            return ris_error::new_result!("member to exist");
        };

        let Some(JsonValue::Array(times)) = object.get::<JsonValue>("times") else {
            return ris_error::new_result!("member to exist");
        };

        ris_error::assert!(iters.len() == times.len())?;

        let mut data = Vec::new();
        for (i, iter) in iters.iter().enumerate() {
            let time = &times[i];
            let sample = Sample {
                iter: f64::try_from(iter)? as usize,
                time: time.try_into()?,
            };
            data.push(sample);
        }

        Ok(Self {
            sampling_mode,
            data,
        })
    }
}

impl Tukey {
    fn deserialize(value: JsonValue) -> RisResult<Self> {
        let JsonValue::Array(array) = value else {
            return ris_error::new_result!("json was not an array");
        };

        let mut result = Vec::new();

        for value in array.iter() {
            result.push(value.try_into()?);
        }

        Ok(Self(result))
    }
}

//==============================================================================
// Construct Report
//==============================================================================
impl Report {
    pub fn deserialize(path: impl AsRef<Path>) -> RisResult<Self> {
        let path = path.as_ref();

        eprintln!("DEBUG: deserialize {}", path.display());

        let benchmark_filepath = path.join("new").join("benchmark.json");
        let estimates_filepath = path.join("new").join("estimates.json");
        let raw_filepath = path.join("new").join("raw.csv");
        let sample_filepath = path.join("new").join("sample.json");
        let tukey_filepath = path.join("new").join("tukey.json");

        // read benchmark
        let file_content = std::fs::read_to_string(benchmark_filepath)?;
        let json = JsonValue::deserialize(file_content)?;
        let benchmark = Benchmark::deserialize(json)?;

        // read estimates
        let file_content = std::fs::read_to_string(estimates_filepath)?;
        let json = JsonValue::deserialize(file_content)?;
        let estimates = Estimates::deserialize(json)?;

        // read raw
        let file_content = std::fs::read_to_string(raw_filepath)?;
        let raw = Raw::deserialize(file_content)?;

        // read sample
        let file_content = std::fs::read_to_string(sample_filepath)?;
        let json = JsonValue::deserialize(file_content)?;
        let sample = Samples::deserialize(json)?;

        // read tukey
        let file_content = std::fs::read_to_string(tukey_filepath)?;
        let json = JsonValue::deserialize(file_content)?;
        let tukey = Tukey::deserialize(json)?;

        Ok(Self {
            benchmark,
            estimates,
            raw,
            sample,
            tukey,
        })
    }
}
