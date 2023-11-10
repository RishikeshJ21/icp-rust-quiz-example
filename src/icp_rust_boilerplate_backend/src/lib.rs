use std::{cell::RefCell, collections::HashMap};

#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Quiz {
    id: u64,
    question: String,
    options: Vec<String>,
    answers: HashMap<String, u32>,
    created_at: u64,
    updated_at: Option<u64>,
}

impl Storable for Quiz {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Quiz {
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

    static STORAGE: RefCell<StableBTreeMap<u64, Quiz, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

impl Quiz {
    fn new(id: u64, question: String, options: Vec<String>) -> Self {
        let answers = options.iter().cloned().map(|option| (option, 0)).collect();
        Self {
            id,
            question,
            options,
            answers,
            created_at: time(),
            updated_at: None,
        }
    }
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct QuizPayload {
    question: String,
    options: Vec<String>,
}

#[ic_cdk::query]
fn get_all_quiz() -> Result<Vec<Quiz>, Error> {
    let quizzes: Vec<Quiz> = STORAGE.with(|service| service.borrow().values().cloned().collect());

    if !quizzes.is_empty() {
        Ok(quizzes)
    } else {
        Err(Error::NotFound {
            msg: "There are currently no quizzes".to_string(),
        })
    }
}

#[ic_cdk::query]
fn get_quiz(id: u64) -> Result<Quiz, Error> {
    _get_quiz(&id).ok_or_else(|| Error::NotFound {
        msg: format!("Quiz with id={} not found", id),
    })
}

fn _get_quiz(id: &u64) -> Option<Quiz> {
    STORAGE.with(|s| s.borrow().get(id))
}

#[ic_cdk::update]
fn create_quiz(payload: QuizPayload) -> Option<Quiz> {
    let id = ID_COUNTER.with(|counter| {
        let current_value = *counter.borrow().get();
        counter
            .borrow_mut()
            .set(current_value + 1)
            .unwrap_or_else(|_| panic!("Cannot increment id counter"))
    });

    let mut answers = HashMap::new();

    for option in &payload.options {
        answers.insert(String::from(option), 0);
    }

    let quiz = Quiz::new(id, payload.question, payload.options);
    do_insert(&quiz);
    Some(quiz)
}

fn do_insert(quiz: &Quiz) {
    STORAGE.with(|service| service.borrow_mut().insert(quiz.id, quiz.clone()));
}

#[ic_cdk::update]
fn update_quiz(id: u64, payload: QuizPayload) -> Result<Quiz, Error> {
    let quiz_option: Option<Quiz> = STORAGE.with(|service| service.borrow().get(&id));

    match quiz_option {
        Some(mut quiz) => {
            let mut answers = HashMap::new();

            for option in &payload.options {
                answers.insert(String::from(option), 0);
            }

            quiz.question = payload.question;
            quiz.options = payload.options;
            quiz.answers = answers;
            quiz.updated_at = Some(time());
            do_insert(&quiz);
            Ok(quiz)
        }
        None => Err(Error::NotFound {
            msg: format!("couldn't update a quiz with id={}. quiz not found", id),
        }),
    }
}

#[ic_cdk::update]
fn delete_quiz(id: u64) -> Result<Quiz, Error> {
    match STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(quiz) => Ok(quiz),
        None => Err(Error::NotFound {
            msg: format!("couldn't delete a quiz with id={}. quiz not found.", id),
        }),
    }
}

#[ic_cdk::update]
fn answer_quiz(id: u64, option: String) -> Result<Quiz, Error> {
    let quiz_option: Option<Quiz> = STORAGE.with(|service| service.borrow().get(&id));

    match quiz_option {
        Some(mut quiz) => {
            if quiz.options.contains(&option) {
                if let Some(answer_count) = quiz.answers.get_mut(&option) {
                    *answer_count += 1;
                }
                quiz.updated_at = Some(time());
                do_insert(&quiz);
                Ok(quiz)
            } else {
                Err(Error::NotFound {
                    msg: format!("The option '{}' is not found for this quiz.", option),
                })
            }
        }
        None => Err(Error::NotFound {
            msg: format!("couldn't cast a quiz with id={}. quiz not found", id),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

ic_cdk::export_candid!();
