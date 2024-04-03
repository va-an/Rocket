use std::sync::atomic::Ordering;

use crate::Tokenizer;

const DEFAULT_SESSION: u64 = 0x726f636b6574;

// #[test]
// fn test_simple_token_validation() -> Result<(), ()> {
//     let tokenizer = Tokenizer::new();
//     let token = tokenizer.form_token(DEFAULT_SESSION);
//     assert!(tokenizer.validate(&token));
//
//     let token_b = tokenizer.form_token(DEFAULT_SESSION);
//     assert!(tokenizer.validate(&token_b));
//     assert_ne!(token, token_b);
//
//     let tokenizer2 = Tokenizer::new();
//     assert!(!tokenizer2.validate(&token));
//     assert!(!tokenizer2.validate(&token_b));
//     Ok(())
// }
//
// #[test]
// fn test_rotation_validation() -> Result<(), ()> {
//     let tokenizer = Tokenizer::new();
//     let token_gen1 = tokenizer.form_token(DEFAULT_SESSION);
//     assert!(tokenizer.validate(&token_gen1));
//
//     tokenizer.rotate();
//     let token_gen2 = tokenizer.form_token(DEFAULT_SESSION);
//     assert!(tokenizer.validate(&token_gen1));
//     assert!(tokenizer.validate(&token_gen2));
//
//     tokenizer.rotate();
//     assert!(!tokenizer.validate(&token_gen1));
//     assert!(tokenizer.validate(&token_gen2));
//
//     tokenizer.rotate();
//     assert!(!tokenizer.validate(&token_gen1));
//     assert!(!tokenizer.validate(&token_gen2));
//     Ok(())
// }
//
// #[test]
// fn test_tokenizer_age_and_generation_progression() -> Result<(), ()> {
//     let tokenizer = Tokenizer::new();
//     assert_eq!(tokenizer.state.load().age.load(Ordering::Relaxed), 0);
//     assert_eq!(tokenizer.state.load().key.generation(), 0);
//
//     let token = tokenizer.form_token(DEFAULT_SESSION);
//     assert!(tokenizer.validate(&token));
//     assert_eq!(tokenizer.state.load().age.load(Ordering::Relaxed), 1);
//     assert_eq!(tokenizer.state.load().key.generation(), 0);
//
//     tokenizer.rotate();
//     assert_eq!(tokenizer.state.load().age.load(Ordering::Relaxed), 0);
//     assert_eq!(tokenizer.state.load().key.generation(), 1);
//
//     let token = tokenizer.form_token(DEFAULT_SESSION);
//     assert!(tokenizer.validate(&token));
//     assert_eq!(tokenizer.state.load().age.load(Ordering::Relaxed), 1);
//     assert_eq!(tokenizer.state.load().key.generation(), 1);
//
//     tokenizer.rotate();
//     assert_eq!(tokenizer.state.load().age.load(Ordering::Relaxed), 0);
//     assert_eq!(tokenizer.state.load().key.generation(), 2);
//     Ok(())
// }
//
// #[test]
// fn test_tokenizer_is_shareable() -> Result<(), ()> {
//     fn is_send_sync<T: Send + Sync + 'static>() {}
//     is_send_sync::<Tokenizer>();
//
//     let tokenizer = Tokenizer::new();
//     let token = tokenizer.form_token(DEFAULT_SESSION);
//     let tokenizer_b = tokenizer.clone();
//
//     assert!(tokenizer.validate(&token));
//     assert!(tokenizer_b.validate(&token));
//     tokenizer_b.rotate();
//
//     assert!(tokenizer.validate(&token));
//     assert!(tokenizer_b.validate(&token));
//
//     tokenizer.rotate();
//
//     assert!(!tokenizer_b.validate(&token));
//     assert!(!tokenizer.validate(&token));
//
//     let token = tokenizer.form_token(DEFAULT_SESSION);
//     drop(tokenizer);
//     assert!(tokenizer_b.validate(&token));
//
//     let tokenizer_c = tokenizer_b.clone();
//     assert!(tokenizer_c.validate(&token));
//     drop(tokenizer_c);
//     assert!(tokenizer_b.validate(&token));
//
//     Ok(())
// }
