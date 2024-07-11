## Settings



## Secrets

> Defining a secret

```yaml
settings:
  [...]
  api_key:
    type: secret
    title: API Key
    optional: false
    help_text: An example of an API key
```
> Setting a secret

```bash
$ screenly edge-app secret set api_key='ABC123'
```
Screenly's secrets function similarly to settings, but with a distinct security model. They are write-only, ensuring they can't be retrieved via the API or web interface once written. To use secrets, you define them in `screenly.yml`, but you do not set a value.

From a consumption perspective (i.e. to use them on the device), secrets are exposed the same way as settings. Thus you can't have a secret and a setting by the same name.

The transmission and storage protocols employ enhanced security. Every Screenly device has its unique pair of public/private keys. For the Screenly Player Max, these keys are securely held in its Trusted Platform Module (TPM), which allows the use of robust x509 cryptography. When we send payload to a Screenly Player, we encrypt it using the device's unique public key, ensuring that only the intended device can decrypt it. Furthermore, secrets on the Player Max are fully encrypted on disk using the TPM, making them inaccessible even if the hard drive is compromised.

For the standard Screenly Player, which doesn't have a TPM, we still utilize robust x509 cryptography with certificates securely stored on disk. While these devices do not offer hardware-level security for stored secrets, our encryption still ensures a high level of protection for your sensitive data.
