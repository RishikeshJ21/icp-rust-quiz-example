#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};
use std::collections::HashMap;

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Voting {
    id: u64,
    question: String,
    options: Vec<String>,
    votes: HashMap<String, u32>,
    created_at: u64,
    updated_at: Option<u64>,
}

// a trait that must be implemented for a struct that is stored in a stable struct
impl Storable for Voting {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

// another trait that must be implemented for a struct that is stored in a stable struct
impl BoundedStorable for Voting {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
        static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
            MemoryManager::init(DefaultMemoryImpl::default())
        );

        static ID_COUNTER: RefCell<IdCell> = RefCell::new(
            IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
                .expect("Cannot create a counter")
        );

        static STORAGE: RefCell<StableBTreeMap<u64, Voting, Memory>> =
            RefCell::new(StableBTreeMap::init(
                MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
        ));
    }

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct VotingPayload {
    question: String,
    options: Vec<String>,
}


#[ic_cdk::query]
fn get_vote(id: u64) -> Result<Voting, Error> {
    match _get_vote(&id) {
        Some(message) => Ok(message),
        None => Err(Error::NotFound {
            msg: format!("a vote with id={} not found", id),
        }),
    }
}

fn _get_vote(id: &u64) -> Option<Voting> {
    STORAGE.with(|s| s.borrow().get(id))
}


#[ic_cdk::update]
fn create_vote(payload: VotingPayload) -> Option<Voting> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter");

    let mut votes = HashMap::new();

    for option in &payload.options {
        votes.insert(String::from(option), 0);
    }


    let vote = Voting {
        id,
        question: payload.question,
        options: payload.options,
        votes,
        created_at: time(),
        updated_at: None,
    };
    do_insert(&vote);
    Some(vote)
}


// helper method to perform insert.
fn do_insert(vote: &Voting) {
    STORAGE.with(|service| service.borrow_mut().insert(vote.id, vote.clone()));
}


#[ic_cdk::update]
fn update_vote(id: u64, payload: VotingPayload) -> Result<Voting, Error> {

    let voting_option: Option<Voting> = STORAGE.with(|service| service.borrow().get(&id));

    match voting_option {

        Some(mut vote) => {


            let mut votes = HashMap::new();

            for option in &payload.options {
                votes.insert(String::from(option), 0);
            }

            vote.question = payload.question;
            vote.options = payload.options;
            vote.votes = votes;
            vote.updated_at = Some(time());
            do_insert(&vote);
            Ok(vote)
        }
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't update a vote with id={}. vote not found",
                id
            ),
        }),
    }
}


#[ic_cdk::update]
fn delete_vote(id: u64) -> Result<Voting, Error> {
    match STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(vote) => Ok(vote),
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't delete a vote with id={}. vote not found.",
                id
            ),
        }),
    }
}


#[ic_cdk::update]
fn cast_vote(id: u64, option: String) -> Result<Voting, Error> {

    let voting_option: Option<Voting> = STORAGE.with(|service| service.borrow().get(&id));

    match voting_option {

        Some(mut vote) => {

            // Check if the selected option is valid
            if vote.options.contains(&option) {
                if let Some(vote_count) = vote.votes.get_mut(&option) {
                    *vote_count += 1;
                }
                vote.updated_at = Some(time());
                do_insert(&vote);
                Ok(vote)
            } else {
                // Return an error if the selected option is not valid
                Err(Error::NotFound {
                    msg: format!("The option '{}' is not found for this vote.", option),
                })
            }
        }
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't cast a vote with id={}. vote not found",
                id
            ),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

// need this to generate candid
ic_cdk::export_candid!();