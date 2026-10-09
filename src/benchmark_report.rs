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
    mean: Estimate,
    median: Estimate,
    median_abs_dev: Estimate,
    slope: Estimate,
    std_dev: Estimate,
}

#[derive(Debug, Clone)]
pub struct Report {
    benchmark: Benchmark,
    estimates: Estimates,
    raw: (),
    sample: (),
    tukey: (),
}

//==============================================================================
// Conversions
//==============================================================================
impl TryFrom<JsonValue> for Benchmark {
    type Error = RisError;

    fn try_from(value: JsonValue) -> RisResult<Self> {
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

impl TryFrom<Option<JsonValue>> for Estimate {
    type Error = RisError;

    fn try_from(value: Option<JsonValue>) -> Result<Self, Self::Error> {
        todo!();
    }
}

impl TryFrom<JsonValue> for Estimates {
    type Error = RisError;

    fn try_from(value: JsonValue) -> Result<Self, Self::Error> {
        let JsonValue::Object(object) = value else {
            return ris_error::new_result!("json was not an object");
        };

        Ok(Self {
            mean: Estimate::try_from(object.get("mean"))?,
            median: Estimate::try_from(object.get("slope"))?,
            median_abs_dev: Estimate::try_from(object.get("median_abs_dev"))?,
            slope: Estimate::try_from(object.get("slope"))?,
            std_dev: Estimate::try_from(object.get("std_dev"))?,
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
        let benchmark = Benchmark::try_from(json)?;

        // read estimates
        let file_content = std::fs::read_to_string(estimates_filepath)?;
        let json = JsonValue::deserialize(file_content)?;
        let estimates = Estimates::try_from(json)?;

        // read raw
        // todo

        // read sample
        // todo

        // read tukey
        // todo

        Ok(Self {
            benchmark,
            estimates,
            raw: (),
            sample: (),
            tukey: (),
        })
    }
}

