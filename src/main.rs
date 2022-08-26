use std::{
    collections::{BTreeMap, HashMap},
    io::{BufRead as _, Write as _},
};

type Sexag = bounded_integer::BoundedU8<0, 59>;

#[derive(Debug, PartialEq, Eq)]
struct BuildItem {
    what: String,
    count: u8,
}

impl std::str::FromStr for BuildItem {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        lazy_static::lazy_static! {
            static ref COUNT_REGEX: regex::Regex = regex::Regex::new(r"^(?P<what>.*) x(?P<count>\d+)$").unwrap();
        }
        if let Some(captures) = COUNT_REGEX.captures(s) {
            return Ok(Self {
                what: captures["what"].to_string(),
                count: captures["count"].parse::<u8>().unwrap(),
            });
        }
        return Ok(Self {
            what: s.to_string(),
            count: 1,
        });
    }
}

#[derive(Debug)]
enum Metadata {
    Supply(Supply),
    SupplyIncrease(Supply),
}

fn should_number(s: &str) -> bool {
    match s {
        "Supply Depot"
        | "SCV"
        | "Missile Turret"
        | "Sensor Tower"
        | "Bunker"
        | "Drone"
        | "Zergling"
        | "Baneling"
        | "Roach"
        | "Ravager"
        | "Hydralisk"
        | "Lurker"
        | "Infestor"
        | "Viper"
        | "Ultralisk"
        | "Swarm Host"
        | "Mutalisk"
        | "Corruptor"
        | "Brood Lord"
        | "Overlord"
        | "Overseer"
        | "Spore Crawler"
        | "Spine Crawler"
        | "Probe"
        | "Probe (Chrono Boost)"
        | "Pylon"
        | "Photon Cannon" => false,
        _ => true,
    }
}

fn batch_duration(s: &str) -> std::time::Duration {
    std::time::Duration::from_secs(match s {
        "Drone" | "Overlord" | "Overseer" | "Zergling" | "Baneling" | "Roach" | "Ravager"
        | "Hydralisk" | "Lurker" | "Infestor" | "Viper" | "Ultralisk" | "Mutalisk"
        | "Corruptor" | "Brood Lord" => 3,
        "SCV" | "Probe" => 10,
        _ => 6,
    })
}

type Supply = bounded_integer::BoundedU8<0, 200>;

fn write_shorthand(writer: &mut impl std::io::Write, supply: Supply) -> std::io::Result<()> {
    if supply < 100 {
        write!(writer, "{}", supply)
    } else {
        write!(writer, "{} {:02}", supply / 100, supply % 100)
    }
}

fn timestamp_to_duration(s: &str) -> std::time::Duration {
    let (minutes, seconds) = s.split_once(':').unwrap();
    let minutes = minutes.parse::<Sexag>().unwrap();
    let seconds = seconds.parse::<Sexag>().unwrap();
    std::time::Duration::from_secs(u64::from(minutes) * 60 + u64::from(seconds))
}

type ByTimestamp = BTreeMap<std::time::Duration, (Vec<BuildItem>, Vec<Metadata>, Option<Supply>)>;

fn main() {
    // Parse file into usable form.
    let arg = std::env::args()
        .skip(1)
        .next()
        .expect("usage: bocleaner <path>");
    let in_file = std::path::Path::new(&arg);
    let filename = in_file
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .trim_start_matches('_');
    let out_file = in_file.with_file_name(filename);
    assert_ne!(out_file, in_file);
    let file = std::fs::File::open(in_file.clone()).expect("open file");
    let annotation_regex = regex::Regex::new(r"^# \((.*)\) (.*)$").unwrap();
    let supply_regex = regex::Regex::new(r"^# \[Supply\] (.*)$").unwrap();
    let reminder_regex = regex::Regex::new(r"^# (\d{1,2}:\d{2}) (.*)$").unwrap();
    let regex = regex::Regex::new(r"^\s*(?P<supply>\d+)\s+(?P<time>\d{1,2}:\d{2})\s+(?P<what>.*)$")
        .unwrap();
    let mut annotations: HashMap<String, std::collections::VecDeque<BuildItem>> = HashMap::new();
    let mut by_timestamp: ByTimestamp = BTreeMap::new();
    let mut per_item: HashMap<String, std::time::Duration> = HashMap::new();
    // Unupgraded CC count. Discard OC or PF items if count is 0.
    let mut cc_count = 1;
    let mut supply_cap = Supply::new(15).unwrap();
    for r in std::io::BufReader::new(file).lines() {
        let line = r.expect("failed to read line");
        let trimmed = line.trim();
        if let Some(captures) = annotation_regex.captures(trimmed) {
            assert_eq!(
                annotations.insert(
                    captures[1].to_string(),
                    captures[2]
                        .split(',')
                        .map(|s| s.trim().parse::<BuildItem>().unwrap())
                        .collect()
                ),
                None
            );
            continue;
        }
        if let Some(captures) = supply_regex.captures(trimmed) {
            for s in captures[1].split(',') {
                let (timestamp, supply) = s
                    .trim()
                    .split_once(' ')
                    .expect(format!("expect 'timestamp supply', got: {}", s).as_str());
                let timestamp = timestamp_to_duration(timestamp);
                let supply = supply.parse::<Supply>().unwrap();
                by_timestamp
                    .entry(timestamp)
                    .or_default()
                    .1
                    .push(Metadata::Supply(supply));
            }
            continue;
        }
        if let Some(captures) = reminder_regex.captures(trimmed) {
            let timestamp = timestamp_to_duration(&captures[1]);
            by_timestamp
                .entry(timestamp)
                .or_default()
                .0
                .push(captures[2].parse::<BuildItem>().unwrap());
        }
        fn add_supply_increase(
            by_timestamp: &mut ByTimestamp,
            complete_time: std::time::Duration,
            supply: Supply,
        ) {
            by_timestamp
                .entry(complete_time)
                .or_default()
                .1
                .push(Metadata::SupplyIncrease(supply));
        }
        if let Some(captures) = regex.captures(trimmed) {
            let supply = captures["supply"].parse::<Supply>().unwrap();
            let timestamp = timestamp_to_duration(&captures["time"]);
            by_timestamp.entry(timestamp).or_default().2.replace(supply);
            let items = captures["what"]
                .split(",")
                .filter_map(|s| {
                    let item = s.trim().parse::<BuildItem>().unwrap();
                    match item.what.as_str() {
                        "Command Center" | "Nexus" => {
                            cc_count += 1;
                            add_supply_increase(
                                &mut by_timestamp,
                                timestamp + std::time::Duration::from_secs(71),
                                Supply::new(15).unwrap().saturating_mul(item.count),
                            );
                        }
                        "Hatchery" => {
                            cc_count += 1;
                            add_supply_increase(
                                &mut by_timestamp,
                                timestamp + std::time::Duration::from_secs(71),
                                Supply::new(6).unwrap().saturating_mul(item.count),
                            );
                        }
                        "Orbital Command" | "Planetary Fortress" => {
                            if cc_count == 0 {
                                println!("ignoring impossible OC/PF: {}", line);
                                return None;
                            }
                            cc_count -= 1;
                        }
                        "Supply Depot" => {
                            add_supply_increase(
                                &mut by_timestamp,
                                timestamp + std::time::Duration::from_secs(21),
                                Supply::new(8).unwrap().saturating_mul(item.count),
                            );
                        }
                        "Overlord" | "Pylon" => {
                            add_supply_increase(
                                &mut by_timestamp,
                                timestamp + std::time::Duration::from_secs(18),
                                Supply::new(8).unwrap().saturating_mul(item.count),
                            );
                        }
                        _ => {}
                    }
                    // Use the presence of an Overlord to know that it's Zerg and adjust
                    // starting supply to 14.
                    if item.what.as_str() == "Overlord" {
                        supply_cap = Supply::new(14).unwrap();
                    }
                    match per_item.entry(item.what.clone()) {
                        std::collections::hash_map::Entry::Occupied(mut occupied) => {
                            let past = *occupied.get();
                            let duration = batch_duration(item.what.as_str());
                            if timestamp - past <= duration {
                                for BuildItem { what, count } in
                                    by_timestamp.get_mut(&past).unwrap().0.iter_mut()
                                {
                                    if *what == item.what {
                                        *count += item.count;
                                        break;
                                    }
                                }
                                None
                            } else {
                                occupied.insert(timestamp);
                                Some(item)
                            }
                        }
                        std::collections::hash_map::Entry::Vacant(vacant) => {
                            vacant.insert(timestamp);
                            Some(item)
                        }
                    }
                })
                .collect::<Vec<_>>();
            if !items.is_empty() {
                by_timestamp.entry(timestamp).or_default().0.extend(items);
            }
        }
    }

    // Output the file.
    // Keep counts per item, write out up to 10.
    // Keep track of supply, overwrite when given an annotation, append value to Depots.
    let mut count_by_item: HashMap<String, u8> = [
        ("Command Center".to_string(), 1),
        ("Hatchery".to_string(), 1),
        ("Nexus".to_string(), 1),
    ]
    .into_iter()
    .collect();
    let mut writer = std::io::BufWriter::new(std::fs::File::create(out_file).unwrap());
    for (timestamp, (items, meta, supply)) in by_timestamp.into_iter() {
        for metadata in meta.into_iter() {
            match metadata {
                Metadata::Supply(dropped) => supply_cap = dropped,
                Metadata::SupplyIncrease(increase) => {
                    supply_cap = supply_cap.saturating_add(increase.into())
                }
            }
        }
        if items.is_empty() {
            continue;
        }
        writer
            .write(
                format!(
                    "   {:>2}:{:02}   ",
                    timestamp.as_secs() / 60,
                    timestamp.as_secs() % 60
                )
                .as_bytes(),
            )
            .unwrap();
        let mut comma = false;
        for BuildItem {
            what,
            count: mut add_count,
        } in items.into_iter()
        {
            if comma {
                writer.write(", ".as_bytes()).unwrap();
            } else {
                comma = true;
            }
            let mut comma = false;
            while add_count > 0 {
                if comma {
                    write!(writer, ", ").unwrap();
                } else {
                    comma = true;
                }
                let (sub_count, annotation) = if let Some(queue) = annotations.get_mut(&what) {
                    if let Some(annotation) = queue.front_mut() {
                        if annotation.count < add_count {
                            let front = queue.pop_front().unwrap();
                            (front.count, front.what)
                        } else {
                            annotation.count -= add_count;
                            let what = annotation.what.clone();
                            if annotation.count == 0 {
                                queue.pop_front();
                            }
                            (add_count, what)
                        }
                    } else {
                        (add_count, "".to_string())
                    }
                } else {
                    (add_count, "".to_string())
                };
                add_count -= sub_count;
                write!(writer, "{}", what).unwrap();
                if should_number(&what) {
                    let count = count_by_item.entry(what.clone()).or_default();
                    let old_count = *count;
                    *count += sub_count;
                    if *count > 1 && *count <= 10 {
                        for i in old_count + 1..=*count {
                            write!(writer, " {}", i).unwrap();
                        }
                    } else if sub_count > 1 {
                        write!(writer, " x{}", sub_count).unwrap();
                    }
                } else if sub_count > 1 {
                    write!(writer, " x{}", sub_count).unwrap();
                }
                if annotation.len() > 0 {
                    write!(writer, " {}", annotation).unwrap();
                }
                match what.as_ref() {
                    "Supply Depot" | "Overlord" | "Pylon" => {
                        if supply_cap < 200 {
                            if let Some(supply) = supply {
                                write!(writer, " ").unwrap();
                                write_shorthand(&mut writer, supply).unwrap();
                                write!(writer, " of").unwrap();
                            }
                            write!(writer, " ").unwrap();
                            write_shorthand(&mut writer, supply_cap).unwrap();
                        }
                    }
                    _ => {}
                }
            }
        }
        write!(writer, "\n").unwrap();
    }
}
