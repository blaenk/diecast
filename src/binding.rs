use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::fmt;

use typemap::TypeMap;

use item::Item;
use configuration::Configuration;

/// Bind data.

#[derive(Clone)]
pub struct Data {
    /// The name of the rule that the bind corresponds to.
    pub name: String,

    /// The bind's dependencies.
    pub dependencies: BTreeMap<String, Arc<Bind>>,

    /// The global configuration
    pub configuration: Arc<Configuration>,

    /// Arbitrary, bind-level data
    pub extensions: Arc<RwLock<TypeMap<::typemap::CloneAny + Sync + Send>>>,
}

impl Data {
    pub fn new(name: String, configuration: Arc<Configuration>) -> Data {
        Data {
            name: name,
            dependencies: BTreeMap::new(),
            configuration: configuration,
            extensions: Arc::new(RwLock::new(TypeMap::custom())),
        }
    }
}

/// The resulting binding of a `Rule`
///
/// `Bind` represents the resulting binding of a particular `Rule`.

#[derive(Clone)]
pub struct Bind {
    items: Vec<Item>,
    data: Arc<Data>,
    is_partial: bool,
}

pub fn set_partial(bind: &mut Bind, is_partial: bool) {
    bind.is_partial = is_partial;
}

impl Bind {
    // FIXME: I don't like that this has to be associated with the configuration
    pub fn new(items: Vec<Item>, data: Arc<Data>) -> Bind {
        Bind {
            items: items,
            data: data,
            is_partial: false,
        }
    }

    /// Whether a bind is out-dated
    pub fn is_partial(&self) -> bool {
        self.is_partial
    }

    /// Mutably access the vector of items.
    ///
    /// This is unsafe because adding items to the vector is
    /// undefined behavior.
    // TODO rename this
    pub unsafe fn all_items_mut(&mut self) -> &mut Vec<Item> {
        &mut self.items
    }

    /// Access the entire set of items mutably
    pub fn items_mut(&mut self) -> &mut [Item] {
        &mut self.items
    }

    /// Access the entire set of items
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Iterate over the items in the binding.
    ///
    /// Note that this possibly only yields the items that have become
    /// outdated. Normally this shouldn't matter. If you do need access
    /// to all of the items, use the `items`/`items_mut` methods.
    pub fn iter<'a>(&'a self) -> Iter<'a> {
        if !self.is_partial {
            Iter {
                iter: IterKind::Full(self.items.iter())
            }
        } else {
            Iter {
                iter: IterKind::Partial(StaleIter {
                    iter: self.items.iter(),
                })
            }
        }
    }

    /// Iterate over the mutable items in the binding.
    ///
    /// Note that this possibly only yields the items that have become
    /// outdated. Normally this shouldn't matter. If you do need access
    /// to all of the items, use the `items`/`items_mut` methods.
    pub fn iter_mut<'a>(&'a mut self) -> IterMut<'a> {
        if !self.is_partial {
            IterMut {
                iter: IterKindMut::Full(self.items.iter_mut())
            }
        } else {
            IterMut {
                iter: IterKindMut::Partial(StaleIterMut {
                    iter: self.items.iter_mut(),
                })
            }
        }
    }

    /// Access the bind data
    pub fn data(&self) -> &Data {
        &self.data
    }

    // TODO audit
    /// Access the bind data as an `Arc`
    pub fn get_data(&self) -> Arc<Data> {
        self.data.clone()
    }
}

struct StaleIter<'a> {
    iter: ::std::slice::Iter<'a, Item>,
}

impl<'a> Iterator for StaleIter<'a> {
    type Item = &'a Item;

    fn next(&mut self) -> Option<&'a Item> {
        while let Some(item) = self.iter.next() {
            if !item.is_stale() {
                continue;
            }

            return Some(item);
        }

        return None;
    }
}

struct StaleIterMut<'a> {
    iter: ::std::slice::IterMut<'a, Item>,
}

impl<'a> Iterator for StaleIterMut<'a> {
    type Item = &'a mut Item;

    fn next(&mut self) -> Option<&'a mut Item> {
        while let Some(item) = self.iter.next() {
            if !item.is_stale() {
                continue;
            }

            return Some(item);
        }

        return None;
    }
}

enum IterKind<'a> {
    Full(::std::slice::Iter<'a, Item>),
    Partial(StaleIter<'a>)
}

pub struct Iter<'a> {
    iter: IterKind<'a>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Item;

    fn next(&mut self) -> Option<&'a Item> {
        match self.iter {
            IterKind::Full(ref mut i) => i.next(),
            IterKind::Partial(ref mut i) => i.next(),
        }
    }
}

enum IterKindMut<'a> {
    Full(::std::slice::IterMut<'a, Item>),
    Partial(StaleIterMut<'a>)
}

pub struct IterMut<'a> {
    iter: IterKindMut<'a>,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = &'a mut Item;

    fn next(&mut self) -> Option<&'a mut Item> {
        match self.iter {
            IterKindMut::Full(ref mut i) => i.next(),
            IterKindMut::Partial(ref mut i) => i.next(),
        }
    }
}

impl<'a> IntoIterator for &'a Bind {
    type Item = &'a Item;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Bind {
    type Item = &'a mut Item;
    type IntoIter = IterMut<'a>;

    fn into_iter(self) -> IterMut<'a> {
        self.iter_mut()
    }
}

// TODO update for Partial(items)
impl fmt::Debug for Bind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {:?}", self.data().name, self.items)
    }
}

