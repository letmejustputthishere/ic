#![allow(unused)] // TODO(NNS1-2409): Re-enable once we add code to migrate indexes.

use crate::{
    known_neuron_index::{AddKnownNeuronError, KnownNeuronIndex, RemoveKnownNeuronError},
    neuron_store::NeuronStoreError,
    pb::v1::Neuron,
    storage::Signed32,
    subaccount_index::NeuronSubaccountIndex,
};

use crate::{
    pb::v1::Topic,
    storage::{NeuronIdU64, TopicSigned32},
};
use ic_base_types::PrincipalId;
use ic_nervous_system_governance::index::{
    neuron_following::{
        add_neuron_followees, remove_neuron_followees, HeapNeuronFollowingIndex,
        NeuronFollowingIndex, StableNeuronFollowingIndex,
    },
    neuron_principal::{
        add_neuron_id_principal_ids, remove_neuron_id_principal_ids, NeuronPrincipalIndex,
        StableNeuronPrincipalIndex,
    },
};
use ic_nns_common::pb::v1::NeuronId;
use ic_stable_structures::VectorMemory;
use icp_ledger::Subaccount;
use mockall::predicate::ne;
use std::{
    collections::{BTreeSet, HashSet},
    fmt::{Display, Formatter},
};

// Because many arguments are needed to construct a StableNeuronIndexes,
// there is no natural argument order that StableNeuronIndexes::new would be able to
// follow. Therefore, constructing a StableNeuronIndexes is done like so:
//
//     let stable_neuron_indexes = neurons::StableNeuronIndexesBuilder {
//         subaccount_index: new_memory(...),
//         principal_index: etc,
//         ...
//     }
//     .build()
pub(crate) struct StableNeuronIndexesBuilder<Memory> {
    pub subaccount: Memory,
    pub principal: Memory,
    pub following: Memory,
    pub known_neuron: Memory,
}

impl<Memory> StableNeuronIndexesBuilder<Memory>
where
    Memory: ic_stable_structures::Memory,
{
    pub fn build(self) -> StableNeuronIndexes<Memory> {
        let Self {
            subaccount,
            principal,
            following,
            known_neuron,
        } = self;

        StableNeuronIndexes {
            subaccount: NeuronSubaccountIndex::new(subaccount),
            principal: StableNeuronPrincipalIndex::new(principal),
            following: StableNeuronFollowingIndex::new(following),
            known_neuron: KnownNeuronIndex::new(known_neuron),
        }
    }
}

/// Neuron indexes based on stable storage.
pub(crate) struct StableNeuronIndexes<Memory>
where
    Memory: ic_stable_structures::Memory,
{
    subaccount: NeuronSubaccountIndex<Memory>,
    principal: StableNeuronPrincipalIndex<NeuronIdU64, Memory>,
    following: StableNeuronFollowingIndex<NeuronIdU64, TopicSigned32, Memory>,
    known_neuron: KnownNeuronIndex<Memory>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CorruptedNeuronIndexes {
    pub neuron_id: NeuronIdU64,
    pub indexes: Vec<NeuronIndexDefect>,
}

impl Display for CorruptedNeuronIndexes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let index_defect_reasons: String = self
            .indexes
            .iter()
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        write!(
            f,
            "Neuron indexes for neuron {} are corrupted: {}",
            self.neuron_id, index_defect_reasons
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum NeuronIndexDefect {
    Subaccount { reason: String },
    Principal { reason: String },
    Following { reason: String },
    KnownNeuron { reason: String },
}

impl Display for NeuronIndexDefect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Subaccount { reason } => {
                write!(f, "Neuron subaccount index is corrupted: {}", reason)
            }
            Self::Principal { reason } => {
                write!(f, "Neuron principal index is corrupted: {}", reason)
            }
            Self::Following { reason } => {
                write!(f, "Neuron following index is corrupted: {}", reason)
            }
            Self::KnownNeuron { reason } => {
                write!(f, "Known neuron index is corrupted: {}", reason)
            }
        }
    }
}

/// A common interface for neuron indexes for updating them in a unified way.
pub trait NeuronIndex {
    /// Adds a neuron into indexes. An error signals something might be wrong with the indexes. The
    /// second time the exact same neuron gets added should have no effect (other than returning an
    /// error).
    fn add_neuron(&mut self, new_neuron: &Neuron) -> Result<(), NeuronIndexDefect>;

    /// Removes a neuron from indexes. An error signals something might be wrong with the indexes.
    /// The second time the exact same neuron gets removed should have no effect (other than
    /// returning an error).
    fn remove_neuron(&mut self, existing_neuron: &Neuron) -> Result<(), NeuronIndexDefect>;

    /// Updates a neuron in the indexes given both of old/new versions. Their neuron ids must be the
    /// same, and it must be checked before calling this method. When it fails, it can return up to
    /// 2 defect entries - one for removing old values and one for adding new values. Encountering
    /// one does not stop performing the other, because there is no good way of recovering from such
    /// errors.
    fn update_neuron(
        &mut self,
        old_neuron: &Neuron,
        new_neuron: &Neuron,
    ) -> Result<(), Vec<NeuronIndexDefect>>;
}

impl<Memory> NeuronIndex for NeuronSubaccountIndex<Memory>
where
    Memory: ic_stable_structures::Memory,
{
    fn add_neuron(&mut self, new_neuron: &Neuron) -> Result<(), NeuronIndexDefect> {
        // StableNeuronIndexes::add_neuron calls validate_neuron which make sure id and subaccount
        // are valid.
        let neuron_id = new_neuron.id.unwrap();
        let subaccount = new_neuron.subaccount().unwrap();

        self.add_neuron_subaccount(neuron_id, &subaccount)
            .map_err(|error| NeuronIndexDefect::Subaccount {
                reason: error.error_message,
            })
    }

    fn remove_neuron(&mut self, existing_neuron: &Neuron) -> Result<(), NeuronIndexDefect> {
        // StableNeuronIndexes::remove_neuron calls validate_neuron which make sure id is valid.
        let neuron_id = existing_neuron.id.unwrap();
        let subaccount = existing_neuron.subaccount().unwrap();

        self.remove_neuron_subaccount(neuron_id, &subaccount)
            .map_err(|error| NeuronIndexDefect::Subaccount {
                reason: error.error_message,
            })
    }

    fn update_neuron(
        &mut self,
        old_neuron: &Neuron,
        new_neuron: &Neuron,
    ) -> Result<(), Vec<NeuronIndexDefect>> {
        // StableNeuronIndexes::update_neuron checks that the subaccount should not be modified. No
        // need to do anything here.
        Ok(())
    }
}

fn already_present_principal_ids_to_result(
    already_present_principal_ids: Vec<PrincipalId>,
    neuron_id: NeuronIdU64,
) -> Result<(), NeuronIndexDefect> {
    if already_present_principal_ids.is_empty() {
        Ok(())
    } else {
        Err(NeuronIndexDefect::Principal {
            reason: format!(
                "Principals {:?} already present for neuron {}",
                already_present_principal_ids, neuron_id
            ),
        })
    }
}

fn already_absent_principal_ids_to_result(
    already_absent_principal_ids: Vec<PrincipalId>,
    neuron_id: NeuronIdU64,
) -> Result<(), NeuronIndexDefect> {
    if already_absent_principal_ids.is_empty() {
        Ok(())
    } else {
        Err(NeuronIndexDefect::Principal {
            reason: format!(
                "Principals {:?} already absent for neuron {}",
                already_absent_principal_ids, neuron_id
            ),
        })
    }
}

impl<Memory> NeuronIndex for StableNeuronPrincipalIndex<NeuronIdU64, Memory>
where
    Memory: ic_stable_structures::Memory,
{
    fn add_neuron(&mut self, new_neuron: &Neuron) -> Result<(), NeuronIndexDefect> {
        // StableNeuronIndexes::add_neuron calls validate_neuron which make sure id is valid.
        let neuron_id = new_neuron.id.unwrap().id;

        let already_present_principal_ids = add_neuron_id_principal_ids(
            self,
            &neuron_id,
            new_neuron.principal_ids_with_special_permissions(),
        );
        already_present_principal_ids_to_result(already_present_principal_ids, neuron_id)
    }

    fn remove_neuron(&mut self, existing_neuron: &Neuron) -> Result<(), NeuronIndexDefect> {
        // StableNeuronIndexes::remove_neuron calls validate_neuron which make sure id is valid.
        let neuron_id = existing_neuron.id.unwrap().id;

        let already_absent_principal_ids = remove_neuron_id_principal_ids(
            self,
            &neuron_id,
            existing_neuron.principal_ids_with_special_permissions(),
        );
        already_absent_principal_ids_to_result(already_absent_principal_ids, neuron_id)
    }

    fn update_neuron(
        &mut self,
        old_neuron: &Neuron,
        new_neuron: &Neuron,
    ) -> Result<(), Vec<NeuronIndexDefect>> {
        // StableNeuronIndexes::update_neuron calls validate_neuron which make sure id is valid.
        let neuron_id = old_neuron.id.unwrap().id;

        let old_principal_ids = old_neuron
            .principal_ids_with_special_permissions()
            .into_iter()
            .collect::<HashSet<_>>();
        let new_principal_ids = new_neuron
            .principal_ids_with_special_permissions()
            .into_iter()
            .collect::<HashSet<_>>();

        // Set differences are used for preventing excessive stable storage writes, which are expensive especially when they are scattered.
        let principal_ids_to_remove = old_principal_ids
            .difference(&new_principal_ids)
            .cloned()
            .collect::<Vec<_>>();
        let already_absent_principal_ids =
            remove_neuron_id_principal_ids(self, &neuron_id, principal_ids_to_remove);

        let principal_ids_to_add = new_principal_ids
            .difference(&old_principal_ids)
            .cloned()
            .collect::<Vec<_>>();
        let already_present_principal_ids =
            add_neuron_id_principal_ids(self, &neuron_id, principal_ids_to_add);

        let defect_remove =
            already_absent_principal_ids_to_result(already_absent_principal_ids, neuron_id);
        let defect_add =
            already_present_principal_ids_to_result(already_present_principal_ids, neuron_id);

        combine_index_defects(defect_remove, defect_add)
    }
}

fn already_present_topic_followee_pairs_to_result(
    already_present_topic_followee_pairs: Vec<(TopicSigned32, NeuronIdU64)>,
    neuron_id: NeuronIdU64,
) -> Result<(), NeuronIndexDefect> {
    if already_present_topic_followee_pairs.is_empty() {
        Ok(())
    } else {
        Err(NeuronIndexDefect::Following {
            reason: format!(
                "Topic-followee pairs {:?} already exists for neuron {}",
                already_present_topic_followee_pairs, neuron_id
            ),
        })
    }
}

fn already_absent_topic_followee_pairs_to_result(
    already_absent_topic_followee_pairs: Vec<(TopicSigned32, NeuronIdU64)>,
    neuron_id: NeuronIdU64,
) -> Result<(), NeuronIndexDefect> {
    if already_absent_topic_followee_pairs.is_empty() {
        Ok(())
    } else {
        Err(NeuronIndexDefect::Following {
            reason: format!(
                "Topic-followee pairs {:?} already absent for neuron {}",
                already_absent_topic_followee_pairs, neuron_id
            ),
        })
    }
}

fn following_index_add_neuron(
    index: &mut dyn NeuronFollowingIndex<NeuronIdU64, TopicSigned32>,
    new_neuron: &Neuron,
) -> Result<(), NeuronIndexDefect> {
    // StableNeuronIndexes::add_neuron checks neuron id before calling this method.
    let neuron_id = NeuronIdU64::from(new_neuron.id.expect("Neuron must have an id"));
    let already_present_topic_followee_pairs = add_neuron_followees(
        index,
        &neuron_id,
        new_neuron
            .topic_followee_pairs()
            .into_iter()
            .map(|(topic, followee)| (TopicSigned32::from(topic), NeuronIdU64::from(followee)))
            .collect(),
    );

    already_present_topic_followee_pairs_to_result(
        already_present_topic_followee_pairs,
        NeuronIdU64::from(neuron_id),
    )
}

fn following_index_remove_neuron(
    index: &mut dyn NeuronFollowingIndex<NeuronIdU64, TopicSigned32>,
    existing_neuron: &Neuron,
) -> Result<(), NeuronIndexDefect> {
    // StableNeuronIndexes::remove_neuron checks neuron id before calling this method.
    let neuron_id = NeuronIdU64::from(existing_neuron.id.expect("Neuron must have an id"));
    let already_absent_topic_followee_pairs = remove_neuron_followees(
        index,
        &neuron_id,
        existing_neuron
            .topic_followee_pairs()
            .into_iter()
            .map(|(topic, followee)| (TopicSigned32::from(topic), NeuronIdU64::from(followee)))
            .collect(),
    );

    already_absent_topic_followee_pairs_to_result(
        already_absent_topic_followee_pairs,
        NeuronIdU64::from(neuron_id),
    )
}

fn following_index_update_neuron(
    index: &mut dyn NeuronFollowingIndex<NeuronIdU64, TopicSigned32>,
    old_neuron: &Neuron,
    new_neuron: &Neuron,
) -> Result<(), Vec<NeuronIndexDefect>> {
    // StableNeuronIndexes::update_neuron calls validate_neuron which make sure id is valid.
    let neuron_id = NeuronIdU64::from(old_neuron.id.expect("Neuron must have an id"));
    let old_topic_followee_pairs = old_neuron.topic_followee_pairs();
    let new_topic_followee_pairs = new_neuron.topic_followee_pairs();

    // Set differences are used for preventing excessive stable storage writes, which are expensive especially when they are scattered.
    let topic_followee_pairs_to_remove = old_topic_followee_pairs
        .difference(&new_topic_followee_pairs)
        .cloned()
        .map(|(topic, followee)| (TopicSigned32::from(topic), NeuronIdU64::from(followee)))
        .collect::<BTreeSet<_>>();
    let topic_followee_pairs_to_add = new_topic_followee_pairs
        .difference(&old_topic_followee_pairs)
        .cloned()
        .map(|(topic, followee)| (TopicSigned32::from(topic), NeuronIdU64::from(followee)))
        .collect::<BTreeSet<_>>();

    let already_absent_topic_followee_pairs =
        remove_neuron_followees(index, &neuron_id, topic_followee_pairs_to_remove);
    let already_present_topic_followee_pairs =
        add_neuron_followees(index, &neuron_id, topic_followee_pairs_to_add);

    let defect_remove = already_absent_topic_followee_pairs_to_result(
        already_absent_topic_followee_pairs,
        neuron_id,
    );
    let defect_add = already_present_topic_followee_pairs_to_result(
        already_present_topic_followee_pairs,
        neuron_id,
    );

    combine_index_defects(defect_remove, defect_add)
}

impl<Memory> NeuronIndex for StableNeuronFollowingIndex<NeuronIdU64, TopicSigned32, Memory>
where
    Memory: ic_stable_structures::Memory,
{
    fn add_neuron(&mut self, new_neuron: &Neuron) -> Result<(), NeuronIndexDefect> {
        following_index_add_neuron(self, new_neuron)
    }

    fn remove_neuron(&mut self, existing_neuron: &Neuron) -> Result<(), NeuronIndexDefect> {
        following_index_remove_neuron(self, existing_neuron)
    }

    fn update_neuron(
        &mut self,
        old_neuron: &Neuron,
        new_neuron: &Neuron,
    ) -> Result<(), Vec<NeuronIndexDefect>> {
        following_index_update_neuron(self, old_neuron, new_neuron)
    }
}

impl NeuronIndex for HeapNeuronFollowingIndex<NeuronIdU64, TopicSigned32> {
    fn add_neuron(&mut self, new_neuron: &Neuron) -> Result<(), NeuronIndexDefect> {
        following_index_add_neuron(self, new_neuron)
    }

    fn remove_neuron(&mut self, existing_neuron: &Neuron) -> Result<(), NeuronIndexDefect> {
        following_index_remove_neuron(self, existing_neuron)
    }

    fn update_neuron(
        &mut self,
        old_neuron: &Neuron,
        new_neuron: &Neuron,
    ) -> Result<(), Vec<NeuronIndexDefect>> {
        following_index_update_neuron(self, old_neuron, new_neuron)
    }
}

impl<Memory> NeuronIndex for KnownNeuronIndex<Memory>
where
    Memory: ic_stable_structures::Memory,
{
    fn add_neuron(&mut self, new_neuron: &Neuron) -> Result<(), NeuronIndexDefect> {
        let known_neuron_name = match new_neuron.known_neuron_data.as_ref() {
            Some(known_neuron_data) => &known_neuron_data.name,
            // This is fine. Only some (a small number) of Neurons are known.
            None => return Ok(()),
        };
        // StableNeuronIndexes::add_neuron checks neuron id before calling this method.
        let neuron_id = new_neuron.id.expect("Neuron must have an id");

        self.add_known_neuron(known_neuron_name, neuron_id)
            .map_err(|add_known_neuron_error| match add_known_neuron_error {
                // It's caller's responsibility to make sure the known neuron name is within the
                // size limit.
                AddKnownNeuronError::AlreadyExists => NeuronIndexDefect::KnownNeuron {
                    reason: format!(
                        "Failed to add neuron {} to known neuron index because the known \
                        neuron name {} already exists",
                        neuron_id.id, known_neuron_name
                    ),
                },
                AddKnownNeuronError::ExceedsSizeLimit => NeuronIndexDefect::KnownNeuron {
                    reason: format!(
                        "Failed to add neuron {} to known neuron index because the known \
                        neuron name {} exceeds size limit",
                        neuron_id.id, known_neuron_name
                    ),
                },
            })
    }

    fn remove_neuron(&mut self, existing_neuron: &Neuron) -> Result<(), NeuronIndexDefect> {
        // StableNeuronIndexes::remove_neuron checks neuron id before calling this method.
        let neuron_id = existing_neuron.id.expect("Neuron must have an id");
        let known_neuron_name = match existing_neuron.known_neuron_data.as_ref() {
            Some(known_neuron_data) => &known_neuron_data.name,
            // This is fine. Only some (a small number) of Neurons are known.
            None => return Ok(()),
        };

        self.remove_known_neuron(known_neuron_name, neuron_id)
            .map_err(|error| match error {
                RemoveKnownNeuronError::AlreadyAbsent => NeuronIndexDefect::KnownNeuron {
                    reason: format!(
                        "Known neuron name {} cannot be removed as it does not exist",
                        known_neuron_name
                    ),
                },
                RemoveKnownNeuronError::NameExistsWithDifferentNeuronId(existing_neuron_id) => {
                    NeuronIndexDefect::KnownNeuron {
                        reason: format!(
                            "Known neuron name {} exists for a different neuron id {}",
                            known_neuron_name, existing_neuron_id.id
                        ),
                    }
                }
            })
    }

    fn update_neuron(
        &mut self,
        old_neuron: &Neuron,
        new_neuron: &Neuron,
    ) -> Result<(), Vec<NeuronIndexDefect>> {
        let old_known_neuron_name = old_neuron
            .known_neuron_data
            .as_ref()
            .map(|known_neuron_data| &known_neuron_data.name);
        let new_known_neuron_name = new_neuron
            .known_neuron_data
            .as_ref()
            .map(|known_neuron_data| &known_neuron_data.name);

        // The comparison can catch 3 cases: adding/removing/updating a known neuron name.
        if old_known_neuron_name == new_known_neuron_name {
            return Ok(());
        }

        // When any update is needed, we can simply remove and add.
        let defect_remove = self.remove_neuron(old_neuron);
        let defect_add = self.add_neuron(new_neuron);

        combine_index_defects(defect_remove, defect_add)
    }
}

// Combine 2 Result<(), NeuronIndexDefect> into Result<(), Vec<NeuronIndexDefect>>. Returns Ok(()) when both are Ok(()).
fn combine_index_defects(
    defect1: Result<(), NeuronIndexDefect>,
    defect2: Result<(), NeuronIndexDefect>,
) -> Result<(), Vec<NeuronIndexDefect>> {
    let defects: Vec<_> = defect1.err().into_iter().chain(defect2.err()).collect();
    if defects.is_empty() {
        Ok(())
    } else {
        Err(defects)
    }
}

impl<Memory> StableNeuronIndexes<Memory>
where
    Memory: ic_stable_structures::Memory,
{
    /// Adds a neuron into indexes, and returns error whether anything is unexpected (e.g. conflicts with
    /// existing data).
    /// Even when we have error for some indexes, we will keep updating other indexes since there is not
    /// a good way to recover from the errors, and the correctness of the indexes need to depend on the
    /// NeuronStore to call them correctly.
    pub fn add_neuron(&mut self, new_neuron: &Neuron) -> Result<(), NeuronStoreError> {
        validate_neuron(new_neuron)?;
        let mut defects = vec![];

        for index in self.indexes_mut() {
            let defect = index.add_neuron(new_neuron).err();
            defects.extend(defect);
        }

        if defects.is_empty() {
            Ok(())
        } else {
            Err(NeuronStoreError::CorruptedNeuronIndexes(
                CorruptedNeuronIndexes {
                    neuron_id: new_neuron.id.unwrap().id, // We can unwrap because of validate_neuron.
                    indexes: defects,
                },
            ))
        }
    }

    /// Removes a neuron from indexes, and returns error whether anything is unexpected (e.g.
    /// expecting something to have existed but they haven't). Even when we have error for some
    /// indexes, we will keep updating other indexes since there is not a good way to recover from
    /// the errors, and the correctness of the indexes need to depend on the NeuronStore to call
    /// them correctly.
    pub fn remove_neuron(&mut self, existing_neuron: &Neuron) -> Result<(), NeuronStoreError> {
        validate_neuron(existing_neuron)?;
        let mut defects = vec![];

        for index in self.indexes_mut() {
            let defect = index.remove_neuron(existing_neuron).err();
            defects.extend(defect);
        }

        if defects.is_empty() {
            Ok(())
        } else {
            Err(NeuronStoreError::CorruptedNeuronIndexes(
                CorruptedNeuronIndexes {
                    neuron_id: existing_neuron.id.unwrap().id, // We can unwrap because of validate_neuron.
                    indexes: defects,
                },
            ))
        }
    }

    /// Updates a neuron from old_neuron to new_neuron. The old/new versions must be valid (has id
    /// and valid subaccount), and they should have the same ids. Even when we have error for some
    /// indexes, we will keep updating other indexes since there is not a good way to recover from
    /// the errors, and the correctness of the indexes need to depend on the NeuronStore to call
    /// them correctly.
    pub fn update_neuron(
        &mut self,
        old_neuron: &Neuron,
        new_neuron: &Neuron,
    ) -> Result<(), NeuronStoreError> {
        validate_neuron(old_neuron)?;
        validate_neuron(new_neuron)?;

        // We can unwrap because of validate_neuron validates that ids exist.
        let old_neuron_id = old_neuron.id.unwrap();
        let new_neuron_id = new_neuron.id.unwrap();
        if old_neuron_id != new_neuron_id {
            return Err(NeuronStoreError::neuron_id_modified(
                old_neuron_id,
                new_neuron_id,
            ));
        }

        // We can unwrap because of validate_neuron validates that ids exist.
        let old_subaccount = old_neuron.subaccount().unwrap();
        let new_subaccount = new_neuron.subaccount().unwrap();
        // Although it is specific to the subaccount index, since each index update only produces
        // defect and does not stop other indexes, we need to stop any index update since account
        // update is invalid, before any index update happens.
        if old_subaccount != new_subaccount {
            return Err(NeuronStoreError::subaccount_modified(
                old_subaccount,
                new_subaccount,
            ));
        }

        let mut defects = vec![];

        for index in self.indexes_mut() {
            let defect = index
                .update_neuron(old_neuron, new_neuron)
                .err()
                .unwrap_or_default();
            defects.extend(defect);
        }

        if defects.is_empty() {
            Ok(())
        } else {
            Err(NeuronStoreError::CorruptedNeuronIndexes(
                CorruptedNeuronIndexes {
                    neuron_id: old_neuron.id.unwrap().id, // We can unwrap because of validate_neuron.
                    indexes: defects,
                },
            ))
        }
    }

    fn indexes_mut(&mut self) -> Vec<&mut dyn NeuronIndex> {
        vec![
            &mut self.subaccount,
            &mut self.principal,
            &mut self.following,
            &mut self.known_neuron,
        ]
    }

    // It's OK to expose read-only access to all the indexes.
    pub fn subaccount(&self) -> &NeuronSubaccountIndex<Memory> {
        &self.subaccount
    }

    pub fn principal(&self) -> &StableNeuronPrincipalIndex<NeuronIdU64, Memory> {
        &self.principal
    }

    pub fn following(&self) -> &StableNeuronFollowingIndex<NeuronIdU64, TopicSigned32, Memory> {
        &self.following
    }

    pub fn known_neuron(&self) -> &KnownNeuronIndex<Memory> {
        &self.known_neuron
    }
}

// To update neuron indexes we need to ensure that the neurons involved have id and valid
// subaccount. At this time the prost-generated Neuron type cannot ensure the above are satisfied,
// so we validate before performing any index update, and assume those are true when updating each
// index.
fn validate_neuron(neuron: &Neuron) -> Result<(), NeuronStoreError> {
    let neuron_id = neuron.id.ok_or(NeuronStoreError::NeuronIdIsNone)?;
    neuron
        .subaccount()
        .map_err(|_| NeuronStoreError::invalid_subaccount(neuron_id, neuron.account.clone()))?;
    Ok(())
}

pub(crate) fn new_heap_based() -> StableNeuronIndexes<VectorMemory> {
    StableNeuronIndexesBuilder {
        subaccount: VectorMemory::default(),
        principal: VectorMemory::default(),
        following: VectorMemory::default(),
        known_neuron: VectorMemory::default(),
    }
    .build()
}

#[cfg(test)]
mod tests;
