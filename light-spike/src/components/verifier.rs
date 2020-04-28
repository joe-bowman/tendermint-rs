use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::prelude::*;

#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerifierError {
    #[error("invalid light block")]
    InvalidLightBlock(crate::errors::ErrorKind),
}

impl_event!(VerifierError);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerifierInput {
    VerifyLightBlock(LightBlock),
}

impl_event!(VerifierInput);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerifierOutput {
    ValidLightBlock(TrustedState),
}

impl_event!(VerifierOutput);

pub struct Verifier {
    predicates: Box<dyn VerificationPredicates>,
    voting_power_calculator: Box<dyn VotingPowerCalculator>,
    commit_validator: Box<dyn CommitValidator>,
    header_hasher: Box<dyn HeaderHasher>,
}

impl Verifier {
    pub fn new(
        predicates: impl VerificationPredicates + 'static,
        voting_power_calculator: impl VotingPowerCalculator + 'static,
        commit_validator: impl CommitValidator + 'static,
        header_hasher: impl HeaderHasher + 'static,
    ) -> Self {
        Self {
            predicates: Box::new(predicates),
            voting_power_calculator: Box::new(voting_power_calculator),
            commit_validator: Box::new(commit_validator),
            header_hasher: Box::new(header_hasher),
        }
    }

    pub fn verify_light_block(
        &self,
        trusted_state: TrustedState,
        light_block: LightBlock,
        trust_threshold: TrustThreshold,
        trusting_period: Duration,
        now: SystemTime,
    ) -> Result<TrustedState, VerifierError> {
        self.predicates
            .verify_light_block(
                &self.voting_power_calculator,
                &self.commit_validator,
                &self.header_hasher,
                &trusted_state,
                &light_block,
                &trust_threshold,
                trusting_period,
                now,
            )
            .map_err(|e| VerifierError::InvalidLightBlock(e.kind().to_owned()))?;

        let new_trusted_state = TrustedState {
            header: light_block.signed_header.header,
            validators: light_block.validator_set,
        };

        Ok(new_trusted_state)
    }
}