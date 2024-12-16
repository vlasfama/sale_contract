#![cfg_attr(not(feature = "export-abi"), no_main)]
extern crate alloc;
use alloc::string::String;
use alloy_primitives::{Address, U256};
use alloy_sol_types::sol;
use stylus_sdk::call::Call;
use stylus_sdk::call::{call, transfer_eth, MethodError};
use stylus_sdk::contract;
use stylus_sdk::contract::balance;
use stylus_sdk::msg;
use stylus_sdk::storage::{StorageAddress, StorageU256};
use stylus_sdk::{evm, prelude::*};

fn ethereum_encode_address(addr: Address) -> Vec<u8> {
    let mut encoded = vec![0u8; 32];
    encoded[12..32].copy_from_slice(addr.as_slice());
    encoded
}

fn ethereum_encode_u256(val: U256) -> Vec<u8> {
    val.to_be_bytes::<32>().to_vec()
}

fn ethereum_decode_u256(data: &[u8]) -> U256 {
    U256::from_be_bytes::<32>(data.try_into().unwrap_or_else(|_| [0u8; 32]))
}

sol_storage! {
    /// TokenSale Storage Structure
    #[entrypoint]
    pub struct TokenSale {
        /// Address of the ERC20 token being sold
        StorageAddress token_address;

        /// Price per token in wei
        StorageU256 token_price;

        /// Total number of tokens sold
        StorageU256 tokens_sold;

        /// Owner of the TokenSale contract
        StorageAddress owner;
    }
}

sol! {
    event TokensPurchased(address indexed buyer, uint256 amount);
    event SaleEnded(address owner, uint256 tokens_sold);
    error InsufficientFunds(uint256 sent, uint256 required);
    error TokenTransferFailed();
    error Unauthorized();
    error TransferFailed();
}

#[derive(Debug, PartialEq, Eq)]
pub enum TokenSaleError {
    InsufficientFunds(U256, U256),
    TokenTransferFailed(),
    Unauthorized(),
    TransferFailed(),
}

impl From<TokenSaleError> for Vec<u8> {
    fn from(error: TokenSaleError) -> Self {
        match error {
            TokenSaleError::InsufficientFunds(sent, required) => {
                InsufficientFunds { sent, required }.encode()
            }
            TokenSaleError::TokenTransferFailed() => TokenTransferFailed {}.encode(),
            TokenSaleError::Unauthorized() => Unauthorized {}.encode(),
            TokenSaleError::TransferFailed() => TransferFailed {}.encode(),
        }
    }
}

#[external]
impl TokenSale {
    fn call_balance_of(&mut self, token_address: Address, owner: Address) -> U256 {
        // balanceOf(address) selector
        let selector = [0x70, 0xa0, 0x82, 0x31];
        let input = [
            selector.as_slice(),
            ethereum_encode_address(owner).as_slice(),
        ]
        .concat();

        // Use `&mut self` as the context
        match call(self, token_address, &input) {
            Ok(output) => ethereum_decode_u256(&output),
            Err(_) => U256::ZERO, // Handle error case
        }
    }

    fn call_transfer(&mut self, token_address: Address, to: Address, value: U256) -> bool {
        let selector = [0xa9, 0x05, 0x9c, 0xbb];
        let input = [
            selector.as_slice(),
            ethereum_encode_address(to).as_slice(),
            ethereum_encode_u256(value).as_slice(),
        ]
        .concat();

        match call(self, token_address, &input) {
            Ok(output) => output.len() == 32 && output[31] == 1,
            Err(_) => false,
        }
    }

    /// Initializes the TokenSale contract
    pub fn init(&mut self, token_address: Address, token_price: U256) -> Result<(), Vec<u8>> {
        // Ensure init is called only once
        if self.owner.get() != Address::ZERO {
            return Err(TokenSaleError::Unauthorized().into());
        }

        // Validate token_price
        if token_price == U256::ZERO {
            return Err(TokenSaleError::InsufficientFunds(U256::ZERO, U256::ZERO).into());
        }

        // Initialize storage variables
        self.token_address.set(token_address);
        self.token_price.set(token_price);
        self.tokens_sold.set(U256::ZERO);
        self.owner.set(msg::sender());

        Ok(())
    }

    /// Allows users to buy tokens by sending ETH
    pub fn buy_tokens(&mut self, number_of_tokens: U256) -> Result<(), Vec<u8>> {
        // Calculate total cost
        let total_cost = self.token_price.get() * number_of_tokens;

        // Check if enough ETH is sent
        if msg::value() < total_cost {
            return Err(TokenSaleError::InsufficientFunds(msg::value(), total_cost).into());
        }

        let buyer = msg::sender();
        let token_address = self.token_address.get();

        let contract_token_balance = self.call_balance_of(token_address, contract::address());

        if contract_token_balance < number_of_tokens {
            return Err(TokenSaleError::TokenTransferFailed().into());
        }

        // Transfer tokens to the buyer using low-level call
        let transfer_success = self.call_transfer(token_address, buyer, number_of_tokens);
        if !transfer_success {
            return Err(TokenSaleError::TokenTransferFailed().into());
        }

        // Update tokens sold
        self.tokens_sold
            .set(self.tokens_sold.get() + number_of_tokens);

        // Optionally, handle excess ETH sent
        let excess = msg::value() - total_cost;
        if excess > U256::ZERO {
            transfer_eth(buyer, excess).map_err(|_| TokenSaleError::TransferFailed())?;
        }

        // Emit TokensPurchased event
        evm::log(TokensPurchased {
            buyer,
            amount: number_of_tokens,
        });

        Ok(())
    }

    /// Allows the owner to end the sale and withdraw remaining tokens and ETH
    pub fn end_sale(&mut self) -> Result<(), Vec<u8>> {
        // Check if the caller is the owner
        if msg::sender() != self.owner.get() {
            return Err(TokenSaleError::Unauthorized().into());
        }

        let token_address = self.token_address.get();

        // Transfer remaining tokens to the owner using low-level call
        let contract_token_balance = self.call_balance_of(token_address, contract::address());

        if contract_token_balance > U256::ZERO {
            let transfer_success =
                self.call_transfer(token_address, self.owner.get(), contract_token_balance);
            if !transfer_success {
                return Err(TokenSaleError::TokenTransferFailed().into());
            }
        }

        // Transfer remaining ETH to the owner
        let eth_balance = balance();
        if eth_balance > U256::ZERO {
            transfer_eth(self.owner.get(), eth_balance)
                .map_err(|_| TokenSaleError::TransferFailed())?;
        }

        // Emit SaleEnded event
        evm::log(SaleEnded {
            owner: self.owner.get(),
            tokens_sold: self.tokens_sold.get(),
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;
    use alloy_primitives::U256;

    #[test]
    fn test_buy_tokens() {
        let mut token_sale = TokenSale::default();
        let owner = Address::from_low_u64_be(1);
        token_sale.init(owner, U256::from(10)).unwrap();

        let buyer = Address::from_low_u64_be(2);
        msg::mock_sender(buyer);
        msg::mock_value(U256::from(100));

        token_sale.buy_tokens(U256::from(10)).unwrap();
        assert_eq!(token_sale.tokens_sold.get(), U256::from(10));
    }

    #[test]
    fn test_end_sale_unauthorized() {
        let mut token_sale = TokenSale::default();
        let owner = Address::from_low_u64_be(1);
        token_sale.init(owner, U256::from(10)).unwrap();

        let non_owner = Address::from_low_u64_be(2);
        msg::mock_sender(non_owner);

        let result = token_sale.end_sale();
        assert!(result.is_err());
    }
}
