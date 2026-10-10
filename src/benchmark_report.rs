use std::path::Path;
use std::io::Read;

use ris_error::prelude::*;

use crate::json::{JsonObject, JsonValue};

//==============================================================================
// Structs
//==============================================================================
#[derive(Debug, Clone)]
pub struct Benchmark {
    group_id: String,
    function_id: String,
    value_str: Option<String>,
    throughput: Option<String>,
    full_id: String,
    directory_name: String,
    title: String,
}

#[derive(Debug, Clone)]
pub struct ConfidenceInterval {
    confidence_level: f64,
    lower_bound: f64,
    upper_bound: f64,
}

#[derive(Debug, Clone)]
pub struct Estimate {
    confidence_interval: ConfidenceInterval,
    point_estimate: f64,
    standard_error: f64,
}

#[derive(Debug, Clone)]
pub struct Estimates {
    mean: Option<Estimate>,
    median: Option<Estimate>,
    median_abs_dev: Option<Estimate>,
    slope: Option<Estimate>,
    std_dev: Option<Estimate>,
}

#[derive(Debug, Clone)]
pub struct Report {
    benchmark: Benchmark,
    estimates: Estimates,
    raw: (),
    sample: Samples,
    tukey: (),
}

#[derive(Debug, Clone)]
pub struct Sample {
    iter: usize,
    time: f64,
}

#[derive(Debug, Clone)]
pub struct Samples {
    sampling_mode: String,
    data: Vec<Sample>,
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
            directory_name: object.get("directory_name")
                .ris_expect("member to exist")?,
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
            confidence_level: confidence_interval_object.get("confidence_level").ris_expect("member to exist")?,
            lower_bound: confidence_interval_object.get("lower_bound").ris_expect("member to exist")?,
            upper_bound: confidence_interval_object.get("upper_bound").ris_expect("member to exist")?,
        };

        let point_estimate = object.get("point_estimate")
            .ris_expect("member to exist")?;
        let standard_error = object.get("standard_error")
            .ris_expect("member to exist")?;

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
            let sample = Sample{
                iter: f64::try_from(iter)? as usize,
                time: time.try_into()?,
            };
            data.push(sample);
        }

        Ok(Self{
            sampling_mode,
            data,
        })
    }
}

//==============================================================================
// Construct Report
//==============================================================================
impl Report {
    pub fn deserialize(path: impl AsRef<Path>) -> RisResult<Self> {
        let path = path.as_ref();
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
        // todo

        // read sample
        let file_content = std::fs::read_to_string(sample_filepath)?;
        let json = JsonValue::deserialize(file_content)?;
        let sample = Samples::deserialize(json)?;

        // read tukey
        // todo

        Ok(Self {
            benchmark,
            estimates,
            raw: (),
            sample,
            tukey: (),
        })
    }
}

