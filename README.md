# Rust Toolchain Setup
Questo progetto utilizza il file `rust-toolchain.toml` per selezionare automaticamente il toolchain Rust nightly richiesto.

## Prerequisiti
- Assicurati di avere [rustup](https://rustup.rs/) installato. Puoi verificarlo con:
  ```sh
  rustup --version
  ```

## Selezione automatica del toolchain
Quando entri nella directory del progetto, rustup rileva il file `rust-toolchain.toml` e 
seleziona automaticamente la versione nightly. Non è necessario eseguire manualmente `rustup override set nightly`.

Puoi verificare che la nightly sia attiva con:
```sh
rustc --version
```
Dovresti vedere una versione come `rustc 1.xx.x-nightly ...`.

## Risoluzione problemi
- Se la versione nightly non viene attivata, assicurati di:
  - Essere nella directory del progetto.
  - Usare un terminale dove il comando `rustc` è gestito da rustup (controlla il tuo PATH).
  - Non avere override globali o locali che sovrascrivono la scelta del toolchain.
- Se necessario, puoi installare la nightly con:
  ```sh
  rustup toolchain install nightly
  ```

## Ulteriori informazioni
Per dettagli su rustup e la gestione dei toolchain, consulta la [documentazione ufficiale](https://rust-lang.github.io/rustup/).

### Logger su UART
Per vedere i messaggi di log della pico, collegate la pico al PC con il cavo USB e aprite un terminale seriale 
alla velocità di 115200 baud.
I terminali UART vanno collegati con la GP0 (TX) e GP1 (RX) della pico e ovviament a con la massa (GND).
Su linux potete usare il comando:
```
sudo screen /dev/ttyACM0 115200
```
Cercate la tty giusta con il comando:
```
ls /dev/tty*
```

### Pagina di benvenuto
La pagina di benvenuto è raggiungibile all'indirizzo:
```http://<your ip>/.
```
Usata per vedere se funziona il web server.

### Inserimento schema sudoku
L'inserimento avviene dalla pagina:
```
http://<your ip>/upload.
```
Inserire una matrice 9x9 con i numeri da 1 a 9 e '_' per inidicare il numero mancante.
Esempio di riga:
```
  5, 3, _, _, 7, _, _, _, _,
```
Esempio di schema completo da inserire:
```
  5, 3, _, _, 7, _, _, _, _,
  6, _, _, 1, 9, 5, _, _, _,
  _, 9, 8, _, _, _, _, 6, _,
  8, _, _, _, 6, _, _, _, 3,
  4, _, _, 8, _, 3, _, _, 1,
  7, _, _, _, 2, _, _, _, 6,
  _, 6, _, _, _, _, 2, 8, _,
  _, _, _, 4, 1, 9, _, _, 5,
  _, _, _, _, 8, _, _, 7, 9,
```

# Collegamento alla rete Wi-Fi.
Alla partenza la pico si collega di default all'indirizzo IP:
```
192.168.1.115
```
Potete cambiare questo indirizzo modificando il file ```configuration.rs``` cosi come dovete 
cambiare il vostro SSID e la password della rete Wi-Fi a cui volete collegarvi.

Per essere sicuri che la Pico si colleghi alla rete Wi-Fi, potete usare il comando:
```
ping <ip della pico>
```
Se ricevete risposta la pico è collegata alla rete Wi-Fi.
A volte ci mette qualche secondo dalla partenza per collegarsi.
