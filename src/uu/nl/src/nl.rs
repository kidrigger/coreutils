//  * This file is part of the uutils coreutils package.
//  *
//  * (c) Tobias Bohumir Schottdorf <tobias.schottdorf@gmail.com>
//  *
//  * For the full copyright and license information, please view the LICENSE
//  * file that was distributed with this source code.
//  *

use clap::{crate_version, Arg, ArgAction, Command};
use std::fs::File;
use std::io::{stdin, BufRead, BufReader, Read};
use std::path::Path;
use uucore::error::{FromIo, UResult, USimpleError};
use uucore::{format_usage, help_about, help_section, help_usage};

mod helper;

const ABOUT: &str = help_about!("nl.md");
const AFTER_HELP: &str = help_section!("after help", "nl.md");
const USAGE: &str = help_usage!("nl.md");

// Settings store options used by nl to produce its output.
pub struct Settings {
    // The variables corresponding to the options -h, -b, and -f.
    header_numbering: NumberingStyle,
    body_numbering: NumberingStyle,
    footer_numbering: NumberingStyle,
    // The variable corresponding to -d
    section_delimiter: [char; 2],
    // The variables corresponding to the options -v, -i, -l, -w.
    starting_line_number: i64,
    line_increment: i64,
    join_blank_lines: u64,
    number_width: usize, // Used with String::from_char, hence usize.
    // The format of the number and the (default value for)
    // renumbering each page.
    number_format: NumberFormat,
    renumber: bool,
    // The string appended to each line number output.
    number_separator: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            header_numbering: NumberingStyle::None,
            body_numbering: NumberingStyle::All,
            footer_numbering: NumberingStyle::None,
            section_delimiter: ['\\', ':'],
            starting_line_number: 1,
            line_increment: 1,
            join_blank_lines: 1,
            number_width: 6,
            number_format: NumberFormat::Right,
            renumber: true,
            number_separator: String::from("\t"),
        }
    }
}

// NumberingStyle stores which lines are to be numbered.
// The possible options are:
// 1. Number all lines
// 2. Number only nonempty lines
// 3. Don't number any lines at all
// 4. Number all lines that match a basic regular expression.
enum NumberingStyle {
    All,
    NonEmpty,
    None,
    Regex(Box<regex::Regex>),
}

// NumberFormat specifies how line numbers are output within their allocated
// space. They are justified to the left or right, in the latter case with
// the option of having all unused space to its left turned into leading zeroes.
#[derive(Default)]
enum NumberFormat {
    Left,
    #[default]
    Right,
    RightZero,
}

impl<T: AsRef<str>> From<T> for NumberFormat {
    fn from(s: T) -> Self {
        match s.as_ref() {
            "ln" => Self::Left,
            "rn" => Self::Right,
            "rz" => Self::RightZero,
            _ => unreachable!("Should have been caught by clap"),
        }
    }
}

impl NumberFormat {
    // Turns a line number into a `String` with at least `min_width` chars,
    // formatted according to the `NumberFormat`s variant.
    fn format(&self, number: i64, min_width: usize) -> String {
        match self {
            Self::Left => format!("{number:<min_width$}"),
            Self::Right => format!("{number:>min_width$}"),
            Self::RightZero if number < 0 => format!("-{0:0>1$}", number.abs(), min_width - 1),
            Self::RightZero => format!("{number:0>min_width$}"),
        }
    }
}

pub mod options {
    pub const HELP: &str = "help";
    pub const FILE: &str = "file";
    pub const BODY_NUMBERING: &str = "body-numbering";
    pub const SECTION_DELIMITER: &str = "section-delimiter";
    pub const FOOTER_NUMBERING: &str = "footer-numbering";
    pub const HEADER_NUMBERING: &str = "header-numbering";
    pub const LINE_INCREMENT: &str = "line-increment";
    pub const JOIN_BLANK_LINES: &str = "join-blank-lines";
    pub const NUMBER_FORMAT: &str = "number-format";
    pub const NO_RENUMBER: &str = "no-renumber";
    pub const NUMBER_SEPARATOR: &str = "number-separator";
    pub const STARTING_LINE_NUMBER: &str = "starting-line-number";
    pub const NUMBER_WIDTH: &str = "number-width";
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let args = args.collect_lossy();

    let matches = uu_app().try_get_matches_from(args)?;

    let mut settings = Settings::default();

    // Update the settings from the command line options, and terminate the
    // program if some options could not successfully be parsed.
    let parse_errors = helper::parse_options(&mut settings, &matches);
    if !parse_errors.is_empty() {
        return Err(USimpleError::new(
            1,
            format!("Invalid arguments supplied.\n{}", parse_errors.join("\n")),
        ));
    }

    let mut read_stdin = false;
    let files: Vec<String> = match matches.get_many::<String>(options::FILE) {
        Some(v) => v.clone().map(|v| v.to_owned()).collect(),
        None => vec!["-".to_owned()],
    };

    for file in &files {
        if file == "-" {
            // If both file names and '-' are specified, we choose to treat first all
            // regular files, and then read from stdin last.
            read_stdin = true;
            continue;
        }
        let path = Path::new(file);
        let reader = File::open(path).map_err_context(|| file.to_string())?;
        let mut buffer = BufReader::new(reader);
        nl(&mut buffer, &settings)?;
    }

    if read_stdin {
        let mut buffer = BufReader::new(stdin());
        nl(&mut buffer, &settings)?;
    }
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .about(ABOUT)
        .version(crate_version!())
        .override_usage(format_usage(USAGE))
        .after_help(AFTER_HELP)
        .infer_long_args(true)
        .disable_help_flag(true)
        .arg(
            Arg::new(options::HELP)
                .long(options::HELP)
                .help("Print help information.")
                .action(ArgAction::Help),
        )
        .arg(
            Arg::new(options::FILE)
                .hide(true)
                .action(ArgAction::Append)
                .value_hint(clap::ValueHint::FilePath),
        )
        .arg(
            Arg::new(options::BODY_NUMBERING)
                .short('b')
                .long(options::BODY_NUMBERING)
                .help("use STYLE for numbering body lines")
                .value_name("STYLE"),
        )
        .arg(
            Arg::new(options::SECTION_DELIMITER)
                .short('d')
                .long(options::SECTION_DELIMITER)
                .help("use CC for separating logical pages")
                .value_name("CC"),
        )
        .arg(
            Arg::new(options::FOOTER_NUMBERING)
                .short('f')
                .long(options::FOOTER_NUMBERING)
                .help("use STYLE for numbering footer lines")
                .value_name("STYLE"),
        )
        .arg(
            Arg::new(options::HEADER_NUMBERING)
                .short('h')
                .long(options::HEADER_NUMBERING)
                .help("use STYLE for numbering header lines")
                .value_name("STYLE"),
        )
        .arg(
            Arg::new(options::LINE_INCREMENT)
                .short('i')
                .long(options::LINE_INCREMENT)
                .help("line number increment at each line")
                .value_name("NUMBER")
                .value_parser(clap::value_parser!(i64)),
        )
        .arg(
            Arg::new(options::JOIN_BLANK_LINES)
                .short('l')
                .long(options::JOIN_BLANK_LINES)
                .help("group of NUMBER empty lines counted as one")
                .value_name("NUMBER")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new(options::NUMBER_FORMAT)
                .short('n')
                .long(options::NUMBER_FORMAT)
                .help("insert line numbers according to FORMAT")
                .value_name("FORMAT")
                .value_parser(["ln", "rn", "rz"]),
        )
        .arg(
            Arg::new(options::NO_RENUMBER)
                .short('p')
                .long(options::NO_RENUMBER)
                .help("do not reset line numbers at logical pages")
                .action(ArgAction::SetFalse),
        )
        .arg(
            Arg::new(options::NUMBER_SEPARATOR)
                .short('s')
                .long(options::NUMBER_SEPARATOR)
                .help("add STRING after (possible) line number")
                .value_name("STRING"),
        )
        .arg(
            Arg::new(options::STARTING_LINE_NUMBER)
                .short('v')
                .long(options::STARTING_LINE_NUMBER)
                .help("first line number on each logical page")
                .value_name("NUMBER")
                .value_parser(clap::value_parser!(i64)),
        )
        .arg(
            Arg::new(options::NUMBER_WIDTH)
                .short('w')
                .long(options::NUMBER_WIDTH)
                .help("use NUMBER columns for line numbers")
                .value_name("NUMBER")
                .value_parser(clap::value_parser!(usize)),
        )
}

// nl implements the main functionality for an individual buffer.
#[allow(clippy::cognitive_complexity)]
fn nl<T: Read>(reader: &mut BufReader<T>, settings: &Settings) -> UResult<()> {
    let regexp: regex::Regex = regex::Regex::new(r".?").unwrap();
    let mut line_no = settings.starting_line_number;
    let mut empty_line_count: u64 = 0;
    // Initially, we use the body's line counting settings
    let mut regex_filter = match settings.body_numbering {
        NumberingStyle::Regex(ref re) => re,
        _ => &regexp,
    };
    let mut line_filter: fn(&str, &regex::Regex) -> bool = pass_regex;
    for l in reader.lines() {
        let mut l = l.map_err_context(|| "could not read line".to_string())?;
        // Sanitize the string. We want to print the newline ourselves.
        if l.ends_with('\n') {
            l.pop();
        }
        // Next we iterate through the individual chars to see if this
        // is one of the special lines starting a new "section" in the
        // document.
        let line = l;
        let mut odd = false;
        // matched_group counts how many copies of section_delimiter
        // this string consists of (0 if there's anything else)
        let mut matched_groups = 0u8;
        for c in line.chars() {
            // If this is a newline character, the loop should end.
            if c == '\n' {
                break;
            }
            // If we have already seen three groups (corresponding to
            // a header) or the current char does not form part of
            // a new group, then this line is not a segment indicator.
            if matched_groups >= 3 || settings.section_delimiter[usize::from(odd)] != c {
                matched_groups = 0;
                break;
            }
            if odd {
                // We have seen a new group and count it.
                matched_groups += 1;
            }
            odd = !odd;
        }

        // See how many groups we matched. That will tell us if this is
        // a line starting a new segment, and the number of groups
        // indicates what type of segment.
        if matched_groups > 0 {
            // The current line is a section delimiter, so we output
            // a blank line.
            println!();
            // However the line does not count as a blank line, so we
            // reset the counter used for --join-blank-lines.
            empty_line_count = 0;
            match *match matched_groups {
                3 => {
                    // This is a header, so we may need to reset the
                    // line number
                    if settings.renumber {
                        line_no = settings.starting_line_number;
                    }
                    &settings.header_numbering
                }
                1 => &settings.footer_numbering,
                // The only option left is 2, but rust wants
                // a catch-all here.
                _ => &settings.body_numbering,
            } {
                NumberingStyle::All => {
                    line_filter = pass_all;
                }
                NumberingStyle::NonEmpty => {
                    line_filter = pass_nonempty;
                }
                NumberingStyle::None => {
                    line_filter = pass_none;
                }
                NumberingStyle::Regex(ref re) => {
                    line_filter = pass_regex;
                    regex_filter = re;
                }
            }
            continue;
        }
        // From this point on we format and print a "regular" line.
        if line.is_empty() {
            // The line is empty, which means that we have to care
            // about the --join-blank-lines parameter.
            empty_line_count += 1;
        } else {
            // This saves us from having to check for an empty string
            // in the next selector.
            empty_line_count = 0;
        }
        if !line_filter(&line, regex_filter)
            || (empty_line_count > 0 && empty_line_count < settings.join_blank_lines)
        {
            // No number is printed for this line. Either we did not
            // want to print one in the first place, or it is a blank
            // line but we are still collecting more blank lines via
            // the option --join-blank-lines.
            println!("{line}");
            continue;
        }
        // If we make it here, then either we are printing a non-empty
        // line or assigning a line number to an empty line. Either
        // way, start counting empties from zero once more.
        empty_line_count = 0;
        // A line number is to be printed.
        println!(
            "{}{}{}",
            settings
                .number_format
                .format(line_no, settings.number_width),
            settings.number_separator,
            line
        );
        // Now update the line number for the (potential) next
        // line.
        line_no += settings.line_increment;
    }
    Ok(())
}

fn pass_regex(line: &str, re: &regex::Regex) -> bool {
    re.is_match(line)
}

fn pass_nonempty(line: &str, _: &regex::Regex) -> bool {
    !line.is_empty()
}

fn pass_none(_: &str, _: &regex::Regex) -> bool {
    false
}

fn pass_all(_: &str, _: &regex::Regex) -> bool {
    true
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_format() {
        assert_eq!(NumberFormat::Left.format(12, 1), "12");
        assert_eq!(NumberFormat::Left.format(-12, 1), "-12");
        assert_eq!(NumberFormat::Left.format(12, 4), "12  ");
        assert_eq!(NumberFormat::Left.format(-12, 4), "-12 ");

        assert_eq!(NumberFormat::Right.format(12, 1), "12");
        assert_eq!(NumberFormat::Right.format(-12, 1), "-12");
        assert_eq!(NumberFormat::Right.format(12, 4), "  12");
        assert_eq!(NumberFormat::Right.format(-12, 4), " -12");

        assert_eq!(NumberFormat::RightZero.format(12, 1), "12");
        assert_eq!(NumberFormat::RightZero.format(-12, 1), "-12");
        assert_eq!(NumberFormat::RightZero.format(12, 4), "0012");
        assert_eq!(NumberFormat::RightZero.format(-12, 4), "-012");
    }
}
