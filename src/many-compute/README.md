# many-compute

Small Akash integration PoC. 

No blockchain integration.
No gRPC. 
No API.
Nothing fancy here.

Take MANY requests, e.g., from Gwen, and call the Akash binary behind the scene. 

Some deployment data is stored in a persistent store as to not have to request the information from Akash every time.
Please note that this persistent store can become out of sync with the actual state of the Akash deployment.
Still good enough for a PoC.

## Usage 

1. Install Akash CLI v0.2.1 from https://github.com/akash-network/provider/releases/tag/v0.2.1. Make sure it is in your `PATH`.

1. Create/Recover an Akash account. Ask @fmorency for the existing Lifted account mnemonic. Note the Akash address.
    ```bash
    $ provider-services keys add lifted-account
    ```

1. Fund the account with some AKT. Ask @fmorency for some.
1. Start the `many-compute` server
    ```bash
    many-compute --pem some_id.pem --persistent compute.db --state ./staging/compute_state.json5 [AKASH_WALLET_ADDRESS]
    ```

    The server listed on port `8000` by default.
