FROM many_local/repo_build:latest as builder

# ==== STAGE 2 ====
FROM debian:stable as runtime

ARG USER_ID

RUN useradd -u ${USER_ID} -ms /bin/bash many

STOPSIGNAL SIGTERM

USER many
WORKDIR /app

EXPOSE 8000 26658

# Generate the private key.
COPY --from=builder /src/target/release/many-abci /app/many-abci

ENTRYPOINT ["./many-abci"]
