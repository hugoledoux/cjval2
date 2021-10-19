use cjval::CJValidator;

#[macro_use]
extern crate clap;
use serde_json::Value;
use std::path::Path;
use url::Url;

use clap::{App, AppSettings, Arg};

use anyhow::{anyhow, Result};

fn print_errors(lserrs: &Vec<String>) {
    if lserrs.is_empty() {
        println!("ok");
    } else {
        for (i, e) in lserrs.iter().enumerate() {
            println!("  {}. {}", i + 1, e);
        }
    }
}

fn print_warnings(lswarns: &Vec<String>) {
    if lswarns.is_empty() {
        println!("ok");
    } else {
        for (i, e) in lswarns.iter().enumerate() {
            println!("  {}. {}", i + 1, e);
        }
    }
}

fn summary_and_bye(finalresult: i32) {
    println!("\n");
    println!("============ SUMMARY ============");
    if finalresult == -1 {
        println!("❌ File is invalid");
    } else if finalresult == 0 {
        println!("⚠️  File is valid but has warnings");
    } else {
        println!("✅ File is valid");
    }
    println!("=================================");
    std::process::exit(1);
}

fn main() {
    // Enable ANSI support for Windows
    let desc = format!("{} (supports CityJSON v1.1)", crate_description!());
    #[cfg(windows)]
    let _ = ansi_term::enable_ansi_support();
    let app = App::new(crate_name!())
        .setting(AppSettings::ColorAuto)
        .setting(AppSettings::ColoredHelp)
        .setting(AppSettings::DeriveDisplayOrder)
        // .setting(AppSettings::UnifiedHelpMessage)
        .max_term_width(90)
        .version(crate_version!())
        .about(&*desc)
        .arg(
            Arg::with_name("INPUT")
                .required(true)
                .help("CityJSON file to validate."),
        )
        .arg(
            Arg::with_name("path")
                .short("e")
                .long("extensionfile")
                .multiple(true)
                .takes_value(true)
                .help(
                    "Read the CityJSON Extensions files locally. More than one can \
                     be given. Alternatively you can read them locally with '-d'",
                ),
        )
        .arg(
            Arg::with_name("download-extensions")
                .short("d")
                .long("download")
                .takes_value(false)
                .help(
                    "Download the CityJSON Extensions from their given URLs \
                     in the file. Alternatively you can read them locally with '-e'",
                ),
        );

    let matches = app.get_matches();

    let p1 = Path::new(matches.value_of("INPUT").unwrap())
        .canonicalize()
        .unwrap();
    let s1 = std::fs::read_to_string(&matches.value_of("INPUT").unwrap())
        .expect("Couldn't read CityJSON file");
    println!("Input CityJSON file:\n\t- {:?}", p1);

    //-- fetch the Extension schemas
    let mut exts: Vec<String> = Vec::new();
    let mut pexts = Vec::new();
    println!("Extension schemas:");
    //-- download them
    if matches.is_present("download-extensions") {
        let re: Result<Value, _> = serde_json::from_str(&s1);
        if re.is_ok() {
            let v = re.unwrap();
            let j = v.as_object().unwrap();
            if j.contains_key("extensions") {
                let jexts = j["extensions"].as_object().unwrap();
                for key in jexts.keys() {
                    // let u = Url::parse(jexts[key]["url"].as_str().unwrap()).unwrap();
                    let o = download_extension(&jexts[key]["url"].as_str().unwrap());
                    match o {
                        Ok(l) => {
                            exts.push(l);
                            println!("\t- {}.. ok", jexts[key]["url"].as_str().unwrap())
                        }
                        Err(e) => {
                            println!(
                                "\t- {}.. ERROR \n\t{}",
                                jexts[key]["url"].as_str().unwrap(),
                                e
                            );
                            summary_and_bye(-1);
                        }
                    }
                }
            } else {
                println!("\t- NONE");
            }
        } else {
            println!("Error from the string");
        }
    } else {
        //-- use the local files
        if let Some(efiles) = matches.values_of("extensionfiles") {
            let l: Vec<&str> = efiles.collect();
            for s in l {
                let s2 = std::fs::read_to_string(s).expect("Couldn't read Extension file");
                exts.push(s2);
                let p = Path::new(&s);
                pexts.push(p.canonicalize().unwrap());
            }
        }
        if pexts.is_empty() {
            println!("\t- NONE");
        }
        for each in pexts {
            println!("\t- {:?}", each);
        }
    }

    println!("CityJSON schemas:");
    println!("\t- v{} (built-in)", cjval::CITYJSON_VERSION);

    //-- ERRORS
    println!("\n");
    println!("=== JSON syntax ===");
    let re = CJValidator::from_str(&s1, &exts);
    if re.is_err() {
        let s = format!("Invalid JSON file: {:?}", re.as_ref().err().unwrap());
        let e = vec![s];
        print_errors(&e);
        summary_and_bye(-1);
    } else {
        let e: Vec<String> = vec![];
        print_errors(&e);
    }
    let val = re.unwrap();

    //-- validate against schema
    println!("=== CityJSON schemas ===");
    let mut rev = val.validate_schema();
    print_errors(&rev);
    if rev.is_empty() == false {
        summary_and_bye(-1);
    }

    //-- validate Extensions, if any
    println!("=== Extensions schemas ===");
    rev = val.validate_extensions();
    print_errors(&rev);
    if rev.is_empty() == false {
        summary_and_bye(-1);
    }

    println!("=== parent_children_consistency ===");
    rev = val.parent_children_consistency();
    print_errors(&rev);
    if rev.is_empty() == false {
        summary_and_bye(-1);
    }

    println!("=== wrong_vertex_index ===");
    rev = val.wrong_vertex_index();
    print_errors(&rev);
    if rev.is_empty() == false {
        summary_and_bye(-1);
    }

    //-- WARNINGS
    let mut bwarns = false;
    if rev.is_empty() == true {
        println!("=== duplicate_vertices (warnings) ===");
        rev = val.duplicate_vertices();
        print_warnings(&rev);
        if rev.is_empty() == false {
            bwarns = true;
        }
    }

    if rev.is_empty() == true {
        println!("=== extra_root_properties (warnings) ===");
        rev = val.extra_root_properties();
        print_warnings(&rev);
        if rev.is_empty() == false {
            bwarns = true;
        }
    }

    //-- bye-bye
    if bwarns == false {
        summary_and_bye(1);
    } else {
        summary_and_bye(0);
    }
}

#[tokio::main]
async fn download_extension(theurl: &str) -> Result<String> {
    // println!("{:?}", jext);
    let u = Url::parse(theurl).unwrap();
    let res = reqwest::get(u).await?;
    if res.status().is_success() {
        Ok(res.text().await?)
    } else {
        return Err(anyhow!("Cannot download extension schema: {}", theurl));
    }
}
