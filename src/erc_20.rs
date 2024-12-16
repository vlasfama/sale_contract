// erc20.rs

#![cfg_attr(not(feature = "export-abi"), no_main)]
extern crate alloc;

use alloc::string::String;
use alloy_primitives::Address;
use alloy_sol_types::sol;
use core::marker::PhantomData;
use stylus_sdk::call::MethodError;
use stylus_sdk::storage::StorageAddress;
use stylus_sdk::{alloy_primitives::U256, prelude::*};
use stylus_sdk::{evm, msg, prelude::*};
pub trait Erc20Params {
    const NAME: &'static str;
    const SYMBOL: &'static str;
    const DECIMALS: u8;
    const INITIAL_SUPPLY: U256;
}

/// ERC20 Interface defining essential functions
pub trait IERC20 {
    /// Transfers `value` tokens to address `to`
    fn transfer(&mut self, to: Address, value: U256) -> bool;

    /// Returns the token balance of address `owner`
    fn balance_of(&self, owner: Address) -> U256;

    /// Approves `spender` to spend `value` tokens on behalf of the caller
    fn approve(&mut self, spender: Address, value: U256) -> bool;

    /// Returns the remaining number of tokens that `spender` can spend on behalf of `owner`
    fn allowance(&self, owner: Address, spender: Address) -> U256;

    /// Transfers `value` tokens from address `from` to address `to`
    fn transfer_from(&mut self, from: Address, to: Address, value: U256) -> bool;
}

sol_storage! {
    /// ERC20 Storage Structure
    pub struct Erc20<T> {
        /// Mapping from address to balance
        mapping(address => uint256) balances;

        /// Nested mapping from owner to spender to allowance
        mapping(address => mapping(address => uint256)) allowances;

        /// Total supply of the token
        uint256 total_supply;

        /// Owner of the contract (has special permissions)
        StorageAddress owner;

        /// Address authorized to mint new tokens
        StorageAddress minter;

        /// Phantom data for generic type
        PhantomData<T> phantom;
    }
}

sol! {

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);
    error InsufficientBalance(address from, uint256 have, uint256 want);
    error InsufficientAllowance(address owner, address spender, uint256 have, uint256 want);
    error Unauthorized();
    error MintToZeroAddress();
    error BurnFromZeroAddress();
    error TransferFailed();
    error ApprovalFailed();
}

pub enum Erc20Error {
    InsufficientBalance(InsufficientBalance),
    InsufficientAllowance(InsufficientAllowance),
    Unauthorized(),
    MintToZeroAddress(),
    BurnFromZeroAddress(),
    TransferFailed(),
    ApprovalFailed(),
}

impl From<Erc20Error> for Vec<u8> {
    fn from(error: Erc20Error) -> Self {
        match error {
            Erc20Error::InsufficientBalance(insuff_balance) => insuff_balance.encode(),
            Erc20Error::InsufficientAllowance(insuff_allowance) => insuff_allowance.encode(),
            Erc20Error::Unauthorized() => Unauthorized {}.encode(),
            Erc20Error::MintToZeroAddress() => MintToZeroAddress {}.encode(),
            Erc20Error::BurnFromZeroAddress() => BurnFromZeroAddress {}.encode(),
            Erc20Error::TransferFailed() => TransferFailed {}.encode(),
            Erc20Error::ApprovalFailed() => ApprovalFailed {}.encode(),
        }
    }
}

impl<T: Erc20Params> Erc20<T> {
    pub fn _transfer(&mut self, from: Address, to: Address, value: U256) -> Result<(), Erc20Error> {
        let mut sender_balance = self.balances.setter(from);
        let old_sender_balance = sender_balance.get();
        if old_sender_balance < value {
            return Err(Erc20Error::InsufficientBalance(InsufficientBalance {
                from,
                have: old_sender_balance,
                want: value,
            }));
        }
        sender_balance.set(old_sender_balance - value);

        let mut to_balance = self.balances.setter(to);
        let new_to_balance = to_balance.get() + value;
        to_balance.set(new_to_balance);

        evm::log(Transfer { from, to, value });
        Ok(())
    }

    pub fn mint(&mut self, address: Address, value: U256) -> Result<(), Erc20Error> {
        if msg::sender() != self.minter.get() {
            return Err(Erc20Error::Unauthorized());
        }
        if address == Address::ZERO {
            return Err(Erc20Error::MintToZeroAddress());
        }

        let mut balance = self.balances.setter(address);
        let new_balance = balance.get() + value;
        balance.set(new_balance);

        self.total_supply.set(self.total_supply.get() + value);

        evm::log(Transfer {
            from: Address::ZERO,
            to: address,
            value,
        });
        Ok(())
    }

    pub fn burn(&mut self, address: Address, value: U256) -> Result<(), Erc20Error> {
        if address == Address::ZERO {
            return Err(Erc20Error::BurnFromZeroAddress());
        }

        let mut balance = self.balances.setter(address);
        let old_balance = balance.get();
        if old_balance < value {
            return Err(Erc20Error::InsufficientBalance(InsufficientBalance {
                from: address,
                have: old_balance,
                want: value,
            }));
        }
        balance.set(old_balance - value);
        self.total_supply.set(self.total_supply.get() - value);

        evm::log(Transfer {
            from: address,
            to: Address::ZERO,
            value,
        });
        Ok(())
    }

    pub fn init(&mut self, owner: Address) -> Result<(), Erc20Error> {
        if self.owner.get() != Address::ZERO {
            return Err(Erc20Error::Unauthorized());
        }

        self.owner.set(owner);
        self.minter.set(owner); // Initially, the owner is also the minter
        self.total_supply.set(T::INITIAL_SUPPLY);
        self.balances.setter(owner).set(T::INITIAL_SUPPLY);

        evm::log(Transfer {
            from: Address::ZERO,
            to: owner,
            value: T::INITIAL_SUPPLY,
        });
        Ok(())
    }

    pub fn set_minter(&mut self, new_minter: Address) -> Result<(), Erc20Error> {
        if msg::sender() != self.owner.get() {
            return Err(Erc20Error::Unauthorized());
        }
        self.minter.set(new_minter);
        Ok(())
    }
}

#[external]
impl<T: Erc20Params> Erc20<T> {
    pub fn name(&self) -> String {
        T::NAME.into()
    }

    pub fn symbol(&self) -> String {
        T::SYMBOL.into()
    }

    pub fn decimals(&self) -> u8 {
        T::DECIMALS
    }

    pub fn total_supply(&self) -> U256 {
        self.total_supply.get()
    }

    pub fn balance_of(&self, owner: Address) -> U256 {
        self.balances.get(owner)
    }

    pub fn transfer(&mut self, to: Address, value: U256) -> Result<bool, Erc20Error> {
        if to == Address::ZERO {
            return Err(Erc20Error::TransferFailed());
        }
        self._transfer(msg::sender(), to, value)?;
        Ok(true)
    }

    pub fn transfer_from(
        &mut self,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<bool, Erc20Error> {
        if to == Address::ZERO {
            return Err(Erc20Error::TransferFailed());
        }

        let mut sender_allowances = self.allowances.setter(from);
        let mut allowance = sender_allowances.setter(msg::sender());
        let old_allowance = allowance.get();

        if old_allowance < value {
            return Err(Erc20Error::InsufficientAllowance(InsufficientAllowance {
                owner: from,
                spender: msg::sender(),
                have: old_allowance,
                want: value,
            }));
        }

        allowance.set(old_allowance - value);

        self._transfer(from, to, value)?;

        evm::log(Approval {
            owner: from,
            spender: msg::sender(),
            value: old_allowance - value,
        });

        Ok(true)
    }

    pub fn approve(&mut self, spender: Address, value: U256) -> Result<bool, Erc20Error> {
        if spender == Address::ZERO {
            return Err(Erc20Error::ApprovalFailed());
        }

        self.allowances.setter(msg::sender()).insert(spender, value);
        evm::log(Approval {
            owner: msg::sender(),
            spender,
            value,
        });
        Ok(true)
    }

    pub fn allowance(&self, owner: Address, spender: Address) -> U256 {
        self.allowances.getter(owner).get(spender)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;
    use alloy_primitives::U256;

    struct TestErc20Params;
    impl Erc20Params for TestErc20Params {
        const NAME: &'static str = "Test Token";
        const SYMBOL: &'static str = "TST";
        const DECIMALS: u8 = 18;
        const INITIAL_SUPPLY: U256 = U256::from(1_000_000);
    }

    type TestErc20 = Erc20<TestErc20Params>;

    #[test]
    fn test_initialization() {
        let mut erc20 = TestErc20::default();
        let owner = Address::from_low_u64_be(1);
        erc20.init(owner).unwrap();

        assert_eq!(erc20.name(), "Test Token");
        assert_eq!(erc20.symbol(), "TST");
        assert_eq!(erc20.total_supply(), U256::from(1_000_000));
        assert_eq!(erc20.balance_of(owner), U256::from(1_000_000));
    }

    #[test]
    fn test_transfer_success() {
        let mut erc20 = TestErc20::default();
        let owner = Address::from_low_u64_be(1);
        let recipient = Address::from_low_u64_be(2);
        erc20.init(owner).unwrap();

        assert!(erc20.transfer(recipient, U256::from(100)).unwrap());
        assert_eq!(erc20.balance_of(owner), U256::from(999_900));
        assert_eq!(erc20.balance_of(recipient), U256::from(100));
    }
}
