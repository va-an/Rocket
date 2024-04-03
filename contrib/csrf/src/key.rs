use rand::distributions::{Distribution, Standard};

// TODO: Make this thread-safe (ArcSwap internally). Use it to generate a
// rotating session id. Probably remove generation from this?

#[derive(Debug, Clone)]
pub struct Rotatable<T> {
    generation: u32,
    current: T,
    previous: Option<T>,
}

impl<T> Rotatable<T> {
    #[inline(always)]
    pub fn new(value: T) -> Self {
        Self {
            generation: 0,
            current: value,
            previous: None
        }
    }

    #[inline(always)]
    pub fn rotate(&mut self, new: T) {
        let old = std::mem::replace(&mut self.current, new);
        self.previous.replace(old);
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        std::iter::once(&self.current).chain(self.previous.as_ref())
    }
}

impl<T> Rotatable<T>
    where Standard: Distribution<T>
{
    #[inline(always)]
    pub fn generate() -> Self {
        Self::new(rand::random())
    }

    #[inline(always)]
    pub fn generate_and_rotate(&mut self) -> Result<(), ()> {
        self.rotate(Self::generate().current);
        Ok(())
    }
}

impl<T> AsRef<T> for Rotatable<T> {
    fn as_ref(&self) -> &T {
        &self.current
    }
}
