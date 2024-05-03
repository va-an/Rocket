macro_rules! declare_macro {
    ($($name:ident),*) => (
        $(declare_macro!([$] $name);)*
    );

    ([$d:tt] $name:ident) => (
        #[macro_export]
        macro_rules! $name {
            ($d ($token:tt)*) => ({})
        }
    );
}

declare_macro!(log, log_, launch_info, launch_info_, launch_meta, launch_meta_,
    error, error_, info, info_, trace, trace_, debug, debug_, warn, warn_);
