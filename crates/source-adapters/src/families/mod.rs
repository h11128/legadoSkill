//! Seed family plugins (thin stubs + GenericForm).

mod fiction_xchina;
mod generic_form;
mod jieqi_mobile;
mod xunsearch_pid;

pub use fiction_xchina::{fiction_list_xchina_rules, FictionListXchina, FICTION_LIST_XCHINA_RULES};
pub use generic_form::{generic_form_rules, GenericForm, GENERIC_FORM_RULES};
pub use jieqi_mobile::{jieqi_mobile_rules, JieqiMobile, JIEQI_MOBILE_RULES};
pub use xunsearch_pid::{xunsearch_pid_rules, XunsearchPid, XUNSEARCH_PID_RULES};
