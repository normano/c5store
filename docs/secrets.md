# Secrets Storage

Secrets are encrypted configuration values (e.g. private keys, access keys, etc.). Storage and encryption of these values do occur on third party and internal services at many development shops. At Excerion Sun, secrets are encrypted in a one way fashion where secrets can decrypted only by the application, so the configuration, if ever leaked, cannot be decrypted without the private key.

C5Store was developed at Excerion Sun and is part of the configuration delivery process which means it needs to deliver secrets. The question has been how to do it.

Definitely need a way to specify multple private keys which means that several configuration values will have to name the keyName. Ideally minimal api changes are preferred to hide complexity.

## Implementation Ideas

1. Create a function called getSecret(valueName, keyName) in C5Store class and setup the seed configuration file/files with a set of keyName => path to private key.
  - This is pretty weak since the keyName is specified in code while the keyNames are specified in configuration files. Ideally, if key names are specified in configuration files then the keyName should be close to the encrypted configuration value. You would not know if the correct private key is there until you call getSecret, which is not ideal. API changes is also less than ideal.

2. Upon encryption of the configuration value, store the key name next to the encrypted value in the configuration file. When the value is loaded in to c5Store it is decrypted on the fly and cached. The original get(valueName) is used to get the value as usual. The keys would be stored in a designated and possibly configurable folder with the keyName being the file name.
  - This makes minmial changes to c5Store on the public surface. May likely add a set secret decryptor function to inject a way to handle find secret values and decryption process, but this would be set on the maanger object. Also may add a function to set the secret keys directory. It's an interesting way to go about it to hide complexity.

3. Store encrypted data into individal files and refer to them via a secret value provider and associated configuration values.
  - Not keen on secret value providers since it adds more to the api surface or storing encrypted data into individual files. Encrypted data is very much gibberish and putting that alongside human readable values could be jarring.

# Implementation

Number 2 is fine to implement because hiding complexity is the most ideal. The library will decrypt values once they are read with designated keys that were read on disk.

