use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::field::RecordFields;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, FormattedFields};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

const LOG_ENV: &str = "HOARD_LOG";
const EMPTY_PREFIX: &str = "    ";

struct FormatterVisitor {
    message: Option<String>,
    fields: BTreeMap<String, String>,
    is_terse: bool,
}

impl FormatterVisitor {
    fn new(is_terse: bool) -> Self {
        Self {
            message: None,
            fields: BTreeMap::new(),
            is_terse,
        }
    }
}

impl Visit for FormatterVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if self.message.is_none() && field.name() == "message" {
            self.message = Some(format!("{:?}", value));
        } else {
            let value = if self.is_terse {
                format!("{:?}", value)
            } else {
                format!("{:#?}", value)
            };

            self.fields.insert(field.name().to_string(), value);
        }
    }
}

#[derive(Clone)]
struct Formatter {
    max_level: Level,
}

impl<'writer> FormatFields<'writer> for Formatter {
    fn format_fields<R: RecordFields>(
        &self,
        mut writer: Writer<'writer>,
        fields: R,
    ) -> fmt::Result {
        let is_terse = matches!(self.max_level, Level::ERROR | Level::WARN | Level::INFO);
        let mut visitor = FormatterVisitor::new(is_terse);
        fields.record(&mut visitor);

        let empty_prefix_newline = format!("\n{}", EMPTY_PREFIX);
        let longest_name_len = visitor
            .fields
            .iter()
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(0);
        let fields = visitor
            .fields
            .iter()
            .map(|(name, value)| {
                let value = value
                    .split('\n')
                    .collect::<Vec<_>>()
                    .join(&empty_prefix_newline);
                if is_terse {
                    format!("{}={}", name, value)
                } else {
                    let padding = " ".repeat(longest_name_len - name.len());
                    format!("{}{} = {}", padding, name, value)
                }
            })
            .collect::<Vec<_>>();

        let fields_output = if is_terse {
            fields.join(", ")
        } else {
            let fields = fields.join(&format!("\n{}      ", EMPTY_PREFIX));
            if fields.is_empty() {
                fields
            } else {
                format!("\n{}where {}", EMPTY_PREFIX, fields)
            }
        };

        if let Some(msg) = visitor.message {
            write!(writer, "{}", msg)?;
            if is_terse && !fields_output.is_empty() {
                write!(writer, ": ")?;
            }
        }

        write!(writer, "{}", fields_output)
    }
}

impl<S, N> FormatEvent<S, N> for Formatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut writer = writer;
        let metadata = event.metadata();

        // Only show prefix if debug or higher verbosity, or warning/error
        if self.max_level >= Level::DEBUG || metadata.level() != &Level::INFO {
            write!(writer, "{} {}: ", metadata.level(), metadata.target())?;
        }

        // Write message
        // TODO: ANSIfy?
        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)?;

        // Format spans only if tracing verbosity
        if self.max_level == Level::TRACE {
            if let Some(scope) = ctx.event_scope() {
                for span in scope.from_root() {
                    write!(writer, "{}at {}", EMPTY_PREFIX, span.name())?;
                    let ext = span.extensions();
                    let fields = ext
                        .get::<FormattedFields<N>>()
                        .expect("should never be `None`")
                        .to_string();
                    if !fields.is_empty() {
                        // join string matches the above write!()
                        let fields = fields.split('\n').collect::<Vec<_>>().join("\n   ");
                        write!(writer, ": {}", fields)?;
                    }
                    writeln!(writer)?;
                }
            }

            // Add extra newline for easier reading
            writeln!(writer)
        } else {
            Ok(())
        }
    }
}

pub fn get_subscriber() -> impl Subscriber {
    let max_level = {
        let env_str = std::env::var(LOG_ENV).unwrap_or_else(|_| String::new());

        // Get the last item that is only a level
        let level_opt = env_str
            .split(',')
            .map(str::trim)
            .rev()
            .map(FromStr::from_str)
            .find_map(Result::ok);

        level_opt.unwrap_or_else(|| {
            if cfg!(debug_assertions) {
                Level::DEBUG
            } else {
                Level::INFO
            }
        })
    };

    let env_filter = EnvFilter::try_from_env(LOG_ENV)
        .unwrap_or_else(|_| EnvFilter::default().add_directive(max_level.into()));

    FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .event_format(Formatter { max_level })
        .fmt_fields(Formatter { max_level })
        .finish()
}
