use crate::layout::coverage::Coverage;
use otspec::types::*;
use otspec::DeserializationError;
use otspec::Deserialize;
use otspec::Deserializer;
use otspec::ReaderContext;
use otspec::SerializationError;
use otspec::Serialize;
use otspec::Serializer;

use otspec_macros::tables;
use std::collections::BTreeMap;

tables!(
  SingleSubstFormat1 {
    [offset_base]
    uint16 substFormat
    Offset16(Coverage) coverage // Offset to Coverage table, from beginning of substitution subtable
    int16 deltaGlyphID  // Add to original glyph ID to get substitute glyph ID
  }
  SingleSubstFormat2 {
    [offset_base]
    uint16 substFormat
    Offset16(Coverage)  coverage  // Offset to Coverage table, from beginning of substitution subtable
    Counted(uint16)  substituteGlyphIDs // Array of substitute glyph IDs — ordered by Coverage index
  }
);

#[derive(Debug, Clone, PartialEq)]
pub enum SingleSubstInternal {
    Format1(SingleSubstFormat1),
    Format2(SingleSubstFormat2),
}

impl Serialize for SingleSubstInternal {
    fn to_bytes(&self, data: &mut Vec<u8>) -> Result<(), SerializationError> {
        match self {
            SingleSubstInternal::Format1(s) => s.to_bytes(data),
            SingleSubstInternal::Format2(s) => s.to_bytes(data),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
/// A single substitution subtable.
pub struct SingleSubst {
    /// The mapping of input glyph IDs to replacement glyph IDs.
    pub mapping: BTreeMap<uint16, uint16>,
}

impl SingleSubst {
    fn best_format(&self) -> (uint16, i16) {
        let mut delta = 0_i16;
        let mut map = self.mapping.iter();
        let format: u16 = if let Some((&first_left, &first_right)) = map.next() {
            delta = (first_right as i16).wrapping_sub(first_left as i16);
            let mut format = 1;
            for (&left, &right) in map {
                if (left as i16).wrapping_add(delta) != (right as i16) {
                    format = 2;
                    break;
                }
            }
            format
        } else {
            2
        };
        (format, delta)
    }
}

impl Deserialize for SingleSubst {
    fn from_bytes(c: &mut ReaderContext) -> Result<Self, DeserializationError> {
        let mut mapping = BTreeMap::new();
        let fmt = c.peek(2)?;
        match fmt {
            [0x00, 0x01] => {
                let sub: SingleSubstFormat1 = c.de()?;
                for gid in &sub.coverage.as_ref().unwrap().glyphs {
                    mapping.insert(*gid, (*gid as i16 + sub.deltaGlyphID) as u16);
                }
            }
            [0x00, 0x02] => {
                let sub: SingleSubstFormat2 = c.de()?;
                for (gid, newgid) in sub
                    .coverage
                    .as_ref()
                    .unwrap()
                    .glyphs
                    .iter()
                    .zip(sub.substituteGlyphIDs.iter())
                {
                    mapping.insert(*gid, *newgid);
                }
            }
            _ => panic!("Bad single subst format {:?}", fmt),
        }
        Ok(SingleSubst { mapping })
    }
}

impl From<&SingleSubst> for SingleSubstInternal {
    fn from(val: &SingleSubst) -> Self {
        let coverage = Coverage {
            glyphs: val.mapping.keys().copied().collect(),
        };
        let (format, delta) = val.best_format();
        if format == 1 {
            SingleSubstInternal::Format1(SingleSubstFormat1 {
                substFormat: 1,
                coverage: Offset16::to(coverage),
                deltaGlyphID: delta,
            })
        } else {
            SingleSubstInternal::Format2(SingleSubstFormat2 {
                substFormat: 2,
                coverage: Offset16::to(coverage),
                substituteGlyphIDs: val.mapping.values().copied().collect(),
            })
        }
    }
}

impl Serialize for SingleSubst {
    fn to_bytes(&self, data: &mut Vec<u8>) -> Result<(), SerializationError> {
        let ssi: SingleSubstInternal = self.into();
        ssi.to_bytes(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use otspec_macros::Serialize;
    use std::iter::FromIterator;

    macro_rules! btreemap {
        ($($k:expr => $v:expr),* $(,)?) => {
            std::collections::BTreeMap::<_, _>::from_iter(std::array::IntoIter::new([$(($k, $v),)*]))
        };
    }

    #[test]
    fn test_single_subst_1_serde() {
        let subst = SingleSubst {
            mapping: btreemap!(66 => 67, 68 => 69),
        };
        let binary_subst = vec![
            0x00, 0x01, 0x00, 0x06, 0x00, 0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 66, 0x00, 68,
        ];
        let serialized = otspec::ser::to_bytes(&subst).unwrap();
        assert_eq!(serialized, binary_subst);
        let de: SingleSubst = otspec::de::from_bytes(&binary_subst).unwrap();
        assert_eq!(de, subst);
    }

    #[test]
    fn test_single_subst_2_ser() {
        let subst = SingleSubst {
            mapping: btreemap!(34 => 66, 35 => 66, 36  => 66),
        };
        let binary_subst = vec![
            0x00, 0x02, 0x00, 0x0C, 0x00, 0x03, 0x00, 0x42, 0x00, 0x42, 0x00, 0x42, 0x00, 0x01,
            0x00, 0x03, 0x00, 0x22, 0x00, 0x23, 0x00, 0x24,
        ];
        let serialized = otspec::ser::to_bytes(&subst).unwrap();
        assert_eq!(serialized, binary_subst);
        assert_eq!(
            otspec::de::from_bytes::<SingleSubst>(&binary_subst).unwrap(),
            subst
        );
    }

    #[test]
    fn test_single_subst_internal_ser() {
        let subst = SingleSubst {
            mapping: btreemap!(34 => 66, 35 => 66, 36  => 66),
        };
        let subst: SingleSubstInternal = (&subst).into();
        let binary_subst = vec![
            0x00, 0x02, 0x00, 0x0C, 0x00, 0x03, 0x00, 0x42, 0x00, 0x42, 0x00, 0x42, 0x00, 0x01,
            0x00, 0x03, 0x00, 0x22, 0x00, 0x23, 0x00, 0x24,
        ];
        let serialized = otspec::ser::to_bytes(&subst).unwrap();
        assert_eq!(serialized, binary_subst);
    }

    #[derive(Serialize, Debug)]
    pub struct Test {
        pub t1: Offset16<SingleSubstInternal>,
    }

    #[test]
    fn test_single_subst_internal_ser2() {
        let subst = SingleSubst {
            mapping: btreemap!(34 => 66, 35 => 66, 36  => 66),
        };
        let subst: SingleSubstInternal = (&subst).into();
        let test = Test {
            t1: Offset16::to(subst),
        };

        let binary_subst = vec![
            0x00, 0x02, 0x00, 0x02, 0x00, 0x0C, 0x00, 0x03, 0x00, 0x42, 0x00, 0x42, 0x00, 0x42,
            0x00, 0x01, 0x00, 0x03, 0x00, 0x22, 0x00, 0x23, 0x00, 0x24,
        ];
        let serialized = otspec::ser::to_bytes(&test).unwrap();
        assert_eq!(serialized, binary_subst);
    }

    #[derive(Serialize, Debug)]
    pub struct Test2 {
        pub t1: Offset16<SingleSubstFormat2>,
    }

    #[test]
    fn test_single_subst_internal_ser3() {
        let subst = SingleSubst {
            mapping: btreemap!(34 => 66, 35 => 66, 36  => 66),
        };
        let subst: SingleSubstInternal = (&subst).into();
        if let SingleSubstInternal::Format2(s) = subst {
            let test = Test2 {
                t1: Offset16::to(s),
            };

            let binary_subst = vec![
                0x00, 0x02, 0x00, 0x02, 0x00, 0x0C, 0x00, 0x03, 0x00, 0x42, 0x00, 0x42, 0x00, 0x42,
                0x00, 0x01, 0x00, 0x03, 0x00, 0x22, 0x00, 0x23, 0x00, 0x24,
            ];
            let serialized = otspec::ser::to_bytes(&test).unwrap();
            assert_eq!(serialized, binary_subst);
        } else {
            panic!("Wrong format!");
        }
    }
}
