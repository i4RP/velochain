//! Unit tests for the EVM executor.

use super::*;
use alloy_primitives::{Address, U256};

#[test]
fn test_new_executor() {
    let exec = executor::EvmExecutor::new(27181);
    let addr = Address::ZERO;
    assert_eq!(exec.get_balance(addr), U256::ZERO);
    assert_eq!(exec.get_nonce(addr), 0);
}

#[test]
fn test_set_and_get_balance() {
    let mut exec = executor::EvmExecutor::new(27181);
    let addr = Address::repeat_byte(0x01);
    exec.set_balance(addr, U256::from(1_000_000));
    assert_eq!(exec.get_balance(addr), U256::from(1_000_000));
}

#[test]
fn test_set_code_and_get_code() {
    let mut exec = executor::EvmExecutor::new(27181);
    let addr = Address::repeat_byte(0x02);
    let code = vec![0x60, 0x00, 0x60, 0x00, 0xf3]; // PUSH1 0 PUSH1 0 RETURN
    exec.set_code(addr, code.clone());
    let retrieved = exec.get_code(addr);
    assert_eq!(retrieved, code);
}

#[test]
fn test_reset_clears_state() {
    let mut exec = executor::EvmExecutor::new(27181);
    let addr = Address::repeat_byte(0x03);
    exec.set_balance(addr, U256::from(999));
    assert_eq!(exec.get_balance(addr), U256::from(999));
    exec.reset();
    assert_eq!(exec.get_balance(addr), U256::ZERO);
}

#[test]
fn test_game_action_rejected_by_evm() {
    use velochain_primitives::{Keypair, Transaction};
    use velochain_primitives::transaction::GameAction;
    let mut exec = executor::EvmExecutor::new(27181);
    let kp = Keypair::random();
    let tx = Transaction::new_game_action(27181, 0, GameAction::Respawn);
    let signed = tx.sign(&kp).unwrap();
    let result = exec.execute_tx(&signed);
    assert!(result.is_err());
}

#[test]
fn test_simple_value_transfer() {
    use velochain_primitives::{Keypair, Transaction, TxType};
    let mut exec = executor::EvmExecutor::new(27181);
    let kp = Keypair::random();
    let sender = kp.address();
    let recipient = Address::repeat_byte(0xAA);

    // Fund sender
    exec.set_balance(sender, U256::from(1_000_000_000_000u64));

    let tx = Transaction {
        tx_type: TxType::Legacy,
        chain_id: 27181,
        nonce: 0,
        gas_price: Some(0), // zero gas price for testing
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
        gas_limit: 21_000,
        to: Some(recipient),
        value: U256::from(1000),
        input: Vec::new(),
        game_action: None,
    };
    let signed = tx.sign(&kp).unwrap();
    let outcome = exec.execute_tx(&signed).unwrap();
    assert!(outcome.success);
    assert!(outcome.gas_used > 0);
}

#[test]
fn test_simulate_call() {
    let exec = executor::EvmExecutor::new(27181);
    let from = Address::repeat_byte(0x01);
    let to = Address::repeat_byte(0x02);

    // Simple call to empty account should succeed
    let result = exec.simulate_call(from, Some(to), U256::ZERO, vec![], 100_000);
    assert!(result.is_ok());
    let outcome = result.unwrap();
    assert!(outcome.success);
}

#[test]
fn test_estimate_gas_minimum() {
    let exec = executor::EvmExecutor::new(27181);
    let from = Address::repeat_byte(0x01);
    let to = Address::repeat_byte(0x02);

    let gas = exec.estimate_gas(from, Some(to), U256::ZERO, vec![]).unwrap();
    assert!(gas >= 21_000); // Minimum gas for a simple transfer
}

#[test]
fn test_multiple_balance_sets() {
    let mut exec = executor::EvmExecutor::new(27181);
    let addr1 = Address::repeat_byte(0x01);
    let addr2 = Address::repeat_byte(0x02);

    exec.set_balance(addr1, U256::from(100));
    exec.set_balance(addr2, U256::from(200));

    assert_eq!(exec.get_balance(addr1), U256::from(100));
    assert_eq!(exec.get_balance(addr2), U256::from(200));
}
