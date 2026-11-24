//! Passage de la cadence gyro à la cadence PID (`sensors/gyro.c:gyroUpdate`, `gyro_filter_impl.c`), identique de la 4.3 à
//! la 2026.12 : avec le LPF2 en service, la boucle PID prend la **dernière** sortie du LPF2 (appliqué à chaque échantillon
//! gyro) ; sans lui, la **moyenne** des échantillons reçus depuis la boucle précédente (somme float32, compteur entier).

/// Un axe du gyro entre les deux cadences.
#[derive(Clone, Copy, Debug, Default)]
pub struct Decimator {
    /// `downsampleFilterEnabled` : le LPF2 fait la décimation.
    filtered: bool,
    sum: f32,
    count: u8,
}

impl Decimator {
    /// `filtered` : vrai quand le LPF2 gyro est en service (il filtre avant la décimation).
    pub fn new(filtered: bool) -> Self {
        Decimator {
            filtered,
            sum: 0.0,
            count: 0,
        }
    }

    /// Un échantillon à la cadence gyro (déjà passé par le LPF2 quand `filtered`).
    pub fn push(&mut self, x: f32) {
        if self.filtered {
            self.sum = x;
        } else {
            self.sum += x;
            self.count = self.count.wrapping_add(1);
        }
    }

    /// La valeur vue par la boucle PID ; repart de zéro pour la moyenne suivante.
    pub fn take(&mut self) -> f32 {
        let v = if self.filtered {
            self.sum
        } else {
            let v = if self.count != 0 {
                self.sum / f32::from(self.count)
            } else {
                0.0
            };
            self.sum = 0.0;
            v
        };
        self.count = 0;
        v
    }
}
