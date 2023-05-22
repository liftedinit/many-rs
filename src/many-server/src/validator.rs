use coset::CoseSign1;
use many_error::ManyError;
use many_protocol::{RequestMessage, ResponseMessage};

/// A trait for transforming a request.
pub trait RequestValidator {
    /// Validate the envelope, prior to executing the message.
    fn validate_envelope(&self, _envelope: &CoseSign1) -> Result<(), ManyError> {
        Ok(())
    }

    /// Validate the request message after opening the envelope and validating its
    /// signature, but before executing it.
    fn validate_request(&self, _request: &RequestMessage) -> Result<(), ManyError> {
        Ok(())
    }
    fn message_executed(
        &mut self,
        _request_envelope: &CoseSign1,
        _request: &RequestMessage,
        _response: &mut ResponseMessage,
    ) -> Result<(), ManyError> {
        Ok(())
    }
}

impl RequestValidator for () {}

impl<A: RequestValidator + ?Sized> RequestValidator for Box<A> {
    fn validate_envelope(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
        self.as_ref().validate_envelope(envelope)
    }
    fn validate_request(&self, request: &RequestMessage) -> Result<(), ManyError> {
        self.as_ref().validate_request(request)
    }
    fn message_executed(
        &mut self,
        request_envelope: &CoseSign1,
        request: &RequestMessage,
        response: &mut ResponseMessage,
    ) -> Result<(), ManyError> {
        self.as_mut()
            .message_executed(request_envelope, request, response)
    }
}

impl<A, B> RequestValidator for (A, B)
where
    A: RequestValidator,
    B: RequestValidator,
{
    fn validate_envelope(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
        self.0.validate_envelope(envelope)?;
        self.1.validate_envelope(envelope)
    }
    fn validate_request(&self, request: &RequestMessage) -> Result<(), ManyError> {
        self.0.validate_request(request)?;
        self.1.validate_request(request)
    }
    fn message_executed(
        &mut self,
        envelope: &CoseSign1,
        request: &RequestMessage,
        response: &mut ResponseMessage,
    ) -> Result<(), ManyError> {
        self.0.message_executed(envelope, request, response)?;
        self.1.message_executed(envelope, request, response)
    }
}
