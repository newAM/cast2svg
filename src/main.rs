//! Create an animated `svg` from an `asciicast`.
//!
//! Inspired by [svg-term-cli].
//!
//! This was an experimental project that I only needed to work once.
//! There are plenty of bugs, and you should not expect this to work out of the box.
//!
//! I may revisit this and clean it up, but until then you should not expect
//! support for this code; use it for reference only.
//!
//! # Limitations
//!
//! * Unable to render a still frame, asciicast files must have at least 2 events.
//! * Many terminal color commands are ignored.
//! * Many text attributes (italics, underline) are ignored.
//!
//! [svg-term-cli]: https://github.com/marionebl/svg-term-cli

mod asciicast;
mod frame;

use anyhow::Context;
use asciicast::Header;
use clap::{Parser, ValueHint};
use frame::{Frame, Symbol};

use std::io::{BufRead, BufReader, Write};
use std::{collections::BTreeMap, path::PathBuf};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
};
use xmlwriter::{Indent, XmlWriter};

const FONT_SIZE: f64 = 5.0 / 3.0;
const HEIGHT_SCALE: f64 = 21.71;
const WIDTH_SCALE: usize = 10;

/// Create an animated SVG from an ASCIICAST.
#[derive(Debug, Parser)]
#[clap(author, version)]
struct Args {
    /// Input asciicast file.
    #[clap(parse(from_os_str), value_hint=ValueHint::FilePath)]
    input: PathBuf,
    /// Increase logging verbosity.
    #[clap(short, long, parse(from_occurrences))]
    verbose: usize,
    /// Indent the resulting SVG.
    #[clap(long)]
    indent: bool,
    /// Output file path, outputs to stdout if not set.
    #[clap(short, long)]
    output: Option<PathBuf>,
    /// Render with window decorations.
    #[clap(long)]
    window: bool,
    /// Width in columns, defaults to the value in asciicast header.
    #[clap(long)]
    width: Option<usize>,
    /// Height in rows, defaults to the value in the asciicast header.
    #[clap(long)]
    height: Option<usize>,
}

fn viewbox_dimension(dimension: f64) -> String {
    let mut ret: String = format!("{:.3}", dimension / 10.0);
    while ret.ends_with('0') {
        ret.pop();
    }
    if ret.ends_with('.') {
        ret.pop();
    }
    ret
}

fn read_asciicast(path: PathBuf) -> anyhow::Result<(asciicast::Header, Vec<asciicast::Event>)> {
    let file = File::open(&path)
        .with_context(|| format!("Failed to read asciicast from {}", path.to_string_lossy()))?;
    let mut reader: BufReader<File> = BufReader::new(file);

    let header: asciicast::Header = {
        let mut line: String = String::new();
        reader
            .read_line(&mut line)
            .with_context(|| "Failed to read header from asciicast")?;
        serde_json::from_str(line.as_str())
            .with_context(|| "Failed to deserialize header from asciicast")?
    };
    log::debug!("asciicast header = {:#?}", header);

    let mut events: Vec<asciicast::Event> = Vec::new();
    let mut previous_time: f64 = -1.0;
    for (num, line) in reader.lines().enumerate() {
        let line_num = num + 2;
        let data =
            line.with_context(|| format!("Failed to read line {} from asiicast", line_num))?;
        let event: asciicast::Event = serde_json::from_str(data.as_str())
            .with_context(|| format!("Failed to deserialize line {} from asciicast", line_num))?;

        // validate assumption that event times are always increasing
        if event.time() < previous_time {
            return Err(anyhow::anyhow!(
                "asciicast event on line {} went backwards in time",
                line_num
            ));
        } else {
            previous_time = event.time();
        }

        events.push(event)
    }

    let num_events: usize = events.len();
    const MIN_EVENTS: usize = 2;
    if num_events < MIN_EVENTS {
        return Err(anyhow::anyhow!(
            "The asciicast must have at least {} events, found {}",
            MIN_EVENTS,
            num_events
        ));
    }

    Ok((header, events))
}

/// Create a symbol map from an asciicast.
///
/// This function does all the heavy lifting, reconstructing the terminal frame
/// based on the byte data.
///
/// This uses alacritty's [vte] crate to reconstruct the frames.
///
/// The return value is an ordered multimap.
///
/// The key is a symbol, and each value is a vector of frame numbers that the
/// symbol appears in.
///
/// The data is in the multimap to make allow us to deduplicate symbols for each
/// frame that they appear in later on.
///
/// [vte]: https://github.com/alacritty/vte
fn symbol_map(header: &Header, events: &[asciicast::Event]) -> BTreeMap<Symbol, Vec<usize>> {
    let mut frame: Frame = Frame::new(header.width, header.height);
    let mut parser: vte::Parser = vte::Parser::new();
    let mut symbol_map: BTreeMap<Symbol, Vec<usize>> = BTreeMap::new();
    for (event_num, event) in events.iter().enumerate() {
        log::trace!("Event number {}: x={}, y={}", event_num, frame.x, frame.y);
        for byte in event.event_data().as_bytes() {
            parser.advance(&mut frame, *byte)
        }

        frame.insert_symbols(&mut symbol_map, event_num);
    }
    symbol_map
}

/// Color attributes in the `color_map`.
#[derive(Debug, PartialEq, Eq)]
enum ColorAttribute {
    /// Color referenced by CSS class.
    Class,
    /// Color referenced directly.
    Style,
}

impl ColorAttribute {
    fn to_str(&self) -> &str {
        match self {
            ColorAttribute::Class => "class",
            ColorAttribute::Style => "style",
        }
    }
}

/// Create a color map from an asciicast.
///
/// This function gets a little crazy.
/// Colors can referenced either directly, `<text style="fill: #3465a4" ...`
/// or by CSS class `<text class="a" ...`
/// Referencing by CSS class is only more efficient (in terms of file size)
/// if the color is used more than once.
///
/// The output of this is a map with a key of the (R, G, B) values, and a value
/// of the element name, and the element value.
fn color_map(
    symbol_map: &BTreeMap<Symbol, Vec<usize>>,
) -> HashMap<(u8, u8, u8), (ColorAttribute, String)> {
    let mut color_map: HashMap<(u8, u8, u8), (ColorAttribute, String)> = HashMap::new();
    let mut class: String = String::from("a");

    for symbol in symbol_map.keys() {
        let (r, g, b) = symbol.fg.rgb();
        if let Some((attribute, attribute_value)) = color_map.get_mut(&(r, g, b)) {
            // more than one symbol references this color, move to style
            if *attribute == ColorAttribute::Style {
                *attribute = ColorAttribute::Class;
                *attribute_value = class.clone();

                // CSS classes cannot start with numbers.
                // This increments through lowercase letters.
                let last: char = class.pop().unwrap();
                if last == 'z' {
                    class.push_str("aa");
                } else {
                    let mut buf: [u8; 1] = [0];
                    last.encode_utf8(&mut buf);
                    class.push((buf[0] + 1) as char);
                }
            }
        } else {
            color_map.insert(
                (r, g, b),
                (
                    ColorAttribute::Style,
                    format!("fill: #{:02x}{:02x}{:02x}", r, g, b),
                ),
            );
        }
    }
    color_map
}

fn write_text_element(
    svg: &mut XmlWriter,
    color_map: &HashMap<(u8, u8, u8), (ColorAttribute, String)>,
    symbol: &Symbol,
) {
    svg.start_element("text");
    let (atrribute_name, attribute_value) = color_map.get(&symbol.fg.rgb()).unwrap();
    svg.write_attribute(atrribute_name.to_str(), attribute_value);
    if symbol.x != 0 {
        svg.write_attribute_fmt("x", format_args!("{}", symbol.x));
    }
    svg.write_attribute_fmt(
        "y",
        format_args!("{:.2}", (symbol.y as f64) * HEIGHT_SCALE / 10.0 + FONT_SIZE),
    );
    svg.set_preserve_whitespaces(true);
    svg.write_text(&symbol.escaped_text());
    svg.set_preserve_whitespaces(false);
    svg.end_element(); // text
}

fn main() -> anyhow::Result<()> {
    // CLI arguments and logging setup
    let args = Args::parse();
    stderrlog::new()
        .module(module_path!())
        .verbosity(args.verbose)
        .init()
        .unwrap();

    // handle asciicast input
    let (header, events) = read_asciicast(args.input)?;
    let num_events: usize = events.len();
    let first_event_time: f64 = events.first().unwrap().time();
    let last_event_time: f64 = events.last().unwrap().time();
    debug_assert!(
        last_event_time > first_event_time,
        "last event occured before first event"
    );
    let duration: f64 = last_event_time - first_event_time;
    debug_assert!(duration.is_sign_positive());

    // create SVG symbols from the asciicast data
    let symbol_map: BTreeMap<Symbol, Vec<usize>> = symbol_map(&header, &events);
    let color_map: HashMap<(u8, u8, u8), (ColorAttribute, String)> = color_map(&symbol_map);

    // compose the SVG
    let opt = xmlwriter::Options {
        indent: if args.indent {
            Indent::Spaces(4)
        } else {
            Indent::None
        },
        ..xmlwriter::Options::default()
    };
    let mut svg = XmlWriter::new(opt);

    let (width_pad, height_pad) = if args.window { (40, 60.0) } else { (0, 0.0) };

    let header_height: usize = args.height.unwrap_or(header.height);
    let header_width: usize = args.width.unwrap_or(header.width);

    let svg_height: f64 = (header_height as f64) * HEIGHT_SCALE + height_pad;
    let svg_width: usize = header_width * WIDTH_SCALE + width_pad;

    svg.start_element("svg");
    svg.write_attribute_fmt("height", format_args!("{:.2}", svg_height));
    svg.write_attribute_fmt("width", format_args!("{:.2}", svg_width));
    svg.write_attribute("xmlns", "http://www.w3.org/2000/svg");

    svg.start_element("rect");
    svg.write_attribute_fmt("height", format_args!("{:.2}", svg_height));
    svg.write_attribute_fmt("width", format_args!("{:.2}", svg_width));
    if args.window {
        svg.write_attribute("rx", "5");
        svg.write_attribute("ry", "5");
    }
    svg.write_attribute("style", "fill: #262626");
    svg.end_element(); // rect

    if args.window {
        svg.start_element("svg");
        svg.write_attribute("y", "0%");
        svg.write_attribute("x", "0%");
        svg.start_element("circle");
        svg.write_attribute("cx", "20");
        svg.write_attribute("cy", "20");
        svg.write_attribute("r", "6");
        svg.write_attribute("fill", "#ff5f58");
        svg.end_element(); // circle
        svg.start_element("circle");
        svg.write_attribute("cx", "40");
        svg.write_attribute("cy", "20");
        svg.write_attribute("r", "6");
        svg.write_attribute("fill", "#ffbd2e");
        svg.end_element(); // circle
        svg.start_element("circle");
        svg.write_attribute("cx", "60");
        svg.write_attribute("cy", "20");
        svg.write_attribute("r", "6");
        svg.write_attribute("fill", "#18c132");
        svg.end_element(); // circle
        svg.end_element(); // svg
    }

    svg.start_element("svg");
    svg.write_attribute_fmt("height", format_args!("{:.2}", svg_height));
    svg.write_attribute_fmt("width", format_args!("{:.2}", svg_width));
    svg.write_attribute("xmlns", "http://www.w3.org/2000/svg");
    svg.write_attribute("xmlns:xlink", "http://www.w3.org/1999/xlink");
    if args.window {
        svg.write_attribute("x", "15");
        svg.write_attribute("y", "50");
    }
    let viewbox_width: String = viewbox_dimension(svg_width as f64);
    let viewbox_height: String = viewbox_dimension(svg_height);
    svg.write_attribute_fmt(
        "viewBox",
        format_args!("0 0 {} {}", viewbox_width, viewbox_height),
    );

    svg.start_element("g");
    svg.write_attribute("font-family", "Courier New");
    svg.write_attribute_fmt("font-size", format_args!("{:.2}", FONT_SIZE));
    svg.start_element("defs");

    for (symbol_id, (symbol, frames)) in symbol_map.iter().enumerate() {
        debug_assert!(!frames.is_empty());
        if frames.len() > 1 {
            svg.start_element("symbol");
            svg.write_attribute_fmt("id", format_args!("{}", symbol_id));
            write_text_element(&mut svg, &color_map, symbol);
            svg.end_element(); // symbol
        }
    }
    svg.end_element(); // defs
    svg.start_element("g");
    svg.write_attribute_fmt(
        "style",
        format_args!(
            "animation-duration:{}s;\
            animation-iteration-count:infinite;\
            animation-name:l;\
            animation-timing-function:steps(1,end)",
            duration
        ),
    );
    svg.start_element("svg");
    svg.write_attribute_fmt("width", format_args!("{}", num_events * svg_width));

    for frame in 0..num_events {
        svg.start_element("svg");
        let offset: usize = frame * svg_width;
        if frame != 0 {
            svg.write_attribute_fmt("x", format_args!("{}", offset));
        }

        for (symbol_id, (symbol, frames)) in symbol_map.iter().enumerate() {
            if frames.contains(&frame) {
                if frames.len() == 1 {
                    write_text_element(&mut svg, &color_map, symbol);
                } else {
                    svg.start_element("use");
                    svg.write_attribute_fmt("xlink:href", format_args!("#{}", symbol_id));
                    svg.end_element(); // use
                }
            }
        }

        svg.end_element(); // svg
    }

    svg.end_element(); // g
    svg.end_element(); // svg
    svg.end_element(); // g
    svg.start_element("style");
    svg.write_text("@keyframes l{");

    for (event_num, event) in events.iter().enumerate() {
        if event_num != 0 {
            let pct: f64 = ((event.time() - first_event_time) / duration) * 100.0;
            // e.g. "0.62%{transform:translateX(-1280px)}"
            svg.write_text_fmt(format_args!(
                "{:.1}%{{transform:translateX(-{}px)}}",
                pct,
                svg_width * event_num,
            ));
        }
    }
    svg.write_text("}");

    for ((r, g, b), (attribute_name, attribute_value)) in color_map.iter() {
        if *attribute_name == ColorAttribute::Class {
            svg.write_text_fmt(format_args!(
                ".{}{{fill:#{:02x}{:02x}{:02x}}}",
                attribute_value, r, g, b
            ));
        }
    }
    svg.end_element(); // style

    {
        let mut output = match args.output {
            Some(filepath) => {
                if filepath.exists() {
                    std::fs::remove_file(&filepath).with_context(|| {
                        format!(
                            "Failed to remove existing output file: {}",
                            filepath.to_string_lossy()
                        )
                    })?;
                }
                Box::new(
                    OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open(&filepath)
                        .with_context(|| {
                            format!(
                                "Failed to open file for writing: {}",
                                filepath.to_string_lossy()
                            )
                        })?,
                ) as Box<dyn Write>
            }
            None => Box::new(std::io::stdout()),
        };

        output.write_all(&svg.end_document().into_bytes())?;
        output.write_all(&[b'\n'])?;
        output.flush()?;
    }

    Ok(())
}
