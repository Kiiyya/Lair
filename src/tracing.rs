//! Tracking the fetch and build progress, for pretty output, progress bars, analytics, anything.

use std::path::Path;

use crate::descriptor::Descriptor;
use crate::manifest::Manifest;

pub trait ManifestProgress: Send + Sync + 'static {
    type Tr: Tracer;

    fn start(tr: &Self::Tr, desc: &Descriptor) -> Self;

    fn success(self, _manifest: &Manifest) where Self: Sized { }
}

pub enum SourceProgressMethod<'a> {
    AlreadyDownloaded,
    Git { url: &'a str },
}

pub trait SourceProgress: Send + Sync + 'static {
    type Tr: Tracer;

    fn start<'a>(tr: &Self::Tr, desc: &Descriptor, method: SourceProgressMethod<'a>) -> Self;

    fn success(self, _source_path: &Path) where Self: Sized { }
}

pub trait BuildProgress: Send + Sync + 'static {
    type Tr: Tracer;

    fn start(tr: &Self::Tr, desc: &Descriptor) -> Self;

    fn command(&self, _command: &str) { }

    fn success(self, _ttc_path: &Path) where Self: Sized { }
}

pub trait Tracer: Send + Sync + 'static {
    type Manifest: ManifestProgress<Tr = Self>;
    type Source: SourceProgress<Tr = Self>;
    type Build: BuildProgress<Tr = Self>;

    /// Exploring the dependency tree, we have found a new dependency.
    fn new_descriptor(&self, _desc: &Descriptor) {}

    fn fetching_manifest(&self, desc: &Descriptor) -> Self::Manifest {
        Self::Manifest::start(self, desc)
    }

    fn fetching_repo<'a>(&self, desc: &Descriptor, method: SourceProgressMethod<'a>) -> Self::Source {
        Self::Source::start(self, desc, method)
    }

    fn building(&self, desc: &Descriptor) -> Self::Build {
        Self::Build::start(self, desc)
    }
}

pub mod no_tracing {
    use std::marker::PhantomData;

    use crate::descriptor::Descriptor;

    use super::{Tracer, BuildProgress, SourceProgress, ManifestProgress, SourceProgressMethod};

    #[derive(Debug, Clone, Copy)]
    pub struct Ignore<Tr: Tracer>(PhantomData<Tr>);

    impl<Tr: Tracer> Default for Ignore<Tr> {
        fn default() -> Self {
            Self(PhantomData)
        }
    }

    impl<Tr: Tracer> ManifestProgress for Ignore<Tr> {
        type Tr = Tr;
        fn start(_tr: &Self::Tr, _desc: &Descriptor) -> Self {
            Self::default()
        }
    }
    impl<Tr: Tracer> SourceProgress for Ignore<Tr> {
        type Tr = Tr;
        fn start(_tr: &Self::Tr, _desc: &Descriptor, _method: SourceProgressMethod) -> Self {
            Self::default()
        }
    }
    impl<Tr: Tracer> BuildProgress for Ignore<Tr> {
        type Tr = Tr;
        fn start(_tr: &Self::Tr, _desc: &Descriptor) -> Self {
            Self::default()
        }
    }

    impl Tracer for () {
        type Manifest = Ignore<Self>;
        type Source = Ignore<Self>;
        type Build = Ignore<Self>;
    }
}

pub mod simple {
    use crate::descriptor::Descriptor;

    use super::{Tracer, BuildProgress, SourceProgressMethod, SourceProgress};
    use super::no_tracing::Ignore;

    #[derive(Debug)]
    pub struct SimpleSourceProgress;
    pub struct SimpleBuildProgress;

    impl SourceProgress for SimpleSourceProgress {
        type Tr = SimpleTracer;

        fn start<'a>(_tr: &Self::Tr, desc: &Descriptor, method: SourceProgressMethod<'a>) -> Self {
            match method {
                SourceProgressMethod::AlreadyDownloaded => (),
                SourceProgressMethod::Git { url } => {
                    println!("Downloading {} from {}", desc.name(), url);
                },
            }
            Self
        }
    }

    impl BuildProgress for SimpleBuildProgress {
        type Tr = SimpleTracer;

        fn start(_tr: &Self::Tr, desc: &Descriptor) -> Self {
            println!("Building {}", desc.name());
            Self
        }

        fn command(&self, command: &str) {
            println!("Running command: `{}`", command);
        }
    }

    pub struct SimpleTracer;

    impl Default for SimpleTracer {
        fn default() -> Self { Self }
    }

    impl Tracer for SimpleTracer {
        type Manifest = Ignore<Self>;
        type Source = SimpleSourceProgress;
        type Build = SimpleBuildProgress;
    }
}

