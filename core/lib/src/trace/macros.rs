macro_rules! declare_macro {
    ($($name:ident $level:ident),* $(,)?) => (
        $(declare_macro!([$] $name $level);)*
    );

    ([$d:tt] $name:ident $level:ident) => (
        #[macro_export]
        macro_rules! $name {
            ($d ($t:tt)*) => ($crate::tracing::$level!($d ($t)*));
        }
    );
}

declare_macro!(
    // launch_meta INFO, launch_meta_ INFO,
    error error, error_ error,
    info info, info_ info,
    trace trace, trace_ trace,
    debug debug, debug_ debug,
    warn warn, warn_ warn,
);

macro_rules! declare_span_macro {
    ($($name:ident $level:ident),* $(,)?) => (
        $(declare_span_macro!([$] $name $level);)*
    );

    ([$d:tt] $name:ident $level:ident) => (
        #[macro_export]
        macro_rules! $name {
            ($n:literal $d ([ $d ($f:tt)* ])? => $in_scope:expr) => ({
                $crate::tracing::span!($crate::tracing::Level::$level, $n $d (, $d ($f)* )?)
                    .in_scope(|| $in_scope);
            })
        }
    );
}

declare_span_macro!(error_span ERROR, warn_span WARN,
    info_span INFO, trace_span TRACE, debug_span DEBUG);

macro_rules! span {
    ($level:expr, $($args:tt)*) => {{
        match $level {
            $crate::tracing::Level::ERROR =>
                $crate::tracing::span!($crate::tracing::Level::ERROR, $($args)*),
            $crate::tracing::Level::WARN =>
                $crate::tracing::span!($crate::tracing::Level::WARN, $($args)*),
            $crate::tracing::Level::INFO =>
                $crate::tracing::span!($crate::tracing::Level::INFO, $($args)*),
            $crate::tracing::Level::DEBUG =>
                $crate::tracing::span!($crate::tracing::Level::DEBUG, $($args)*),
            $crate::tracing::Level::TRACE =>
                $crate::tracing::span!($crate::tracing::Level::TRACE, $($args)*),
        }
    }};
}

macro_rules! event {
    ($level:expr, $($args:tt)*) => {{
        match $level {
            $crate::tracing::Level::ERROR => event!(@$crate::tracing::Level::ERROR, $($args)*),
            $crate::tracing::Level::WARN => event!(@$crate::tracing::Level::WARN, $($args)*),
            $crate::tracing::Level::INFO => event!(@$crate::tracing::Level::INFO, $($args)*),
            $crate::tracing::Level::DEBUG => event!(@$crate::tracing::Level::DEBUG, $($args)*),
            $crate::tracing::Level::TRACE => event!(@$crate::tracing::Level::TRACE, $($args)*),
        }
    }};

    (@$level:expr, $n:expr, $($args:tt)*) => {{
        $crate::tracing::event!(name: $n, target: concat!("rocket::", $n), $level, $($args)*);
    }};
}
