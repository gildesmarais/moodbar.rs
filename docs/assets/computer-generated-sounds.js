self.onmessage = function (e) {
  const { type, duration, sampleRate } = e.data;
  const totalSamples = sampleRate * duration;
  const pcm = new Float32Array(totalSamples);

  if (type === "Techno") {
    const bpm = 140;
    const beatLen = 60 / bpm;
    const halfBeat = beatLen / 2;
    const quarterBeat = beatLen / 4;
    const measureLen = beatLen * 4;
    const stepsPerMeasure = 16;

    // Size dynamically based on duration
    const numMeasures = Math.max(8, Math.ceil(duration / measureLen));

    // 16-step patterns represented as binary numbers (16 bits)
    const PATTERN_KICK_STANDARD = 0b1000100010001000;
    const PATTERN_KICK_ACCENTED = 0b1000100110001010;
    const PATTERN_KICK_ROLL = 0b1111111111111111;

    const PATTERN_HAT_OFFBEAT = 0b0010001000100010;
    const PATTERN_HAT_DRIVING = 0b0010101000101010;
    const PATTERN_HAT_CLIMAX = 0b1010101010101010;

    const MELODIES = {
      silent: new Float32Array(16),
      intro: new Float32Array([
        110.0, 0, 110.0, 0, 130.81, 0, 110.0, 0, 110.0, 0, 110.0, 0, 146.83, 0,
        130.81, 0,
      ]),
      main: new Float32Array([
        110.0, 110.0, 130.81, 0, 164.81, 146.83, 110.0, 0, 110.0, 130.81,
        164.81, 196.0, 164.81, 146.83, 110.0, 0,
      ]),
      breakdown: new Float32Array([
        164.81, 196.0, 220.0, 261.63, 293.66, 329.63, 392.0, 440.0, 392.0,
        329.63, 293.66, 261.63, 220.0, 196.0, 164.81, 0,
      ]),
      climax: new Float32Array([
        220.0, 220.0, 261.63, 220.0, 329.63, 293.66, 220.0, 392.0, 220.0,
        261.63, 329.63, 392.0, 329.63, 293.66, 220.0, 0,
      ]),
    };

    // Chords progression (triads) for the mid-frequency pad (Am, Am, C, C, G, G, F, F)
    const CHORDS = [
      [220.0, 261.63, 329.63], // Am (A3, C4, E4)
      [220.0, 261.63, 329.63], // Am
      [261.63, 329.63, 392.0], // C (C4, E4, G4)
      [261.63, 329.63, 392.0], // C
      [196.0, 246.94, 293.66], // G (G3, B3, D4)
      [196.0, 246.94, 293.66], // G
      [174.61, 220.0, 261.63], // F (F3, A3, C4)
      [174.61, 220.0, 261.63], // F
    ];

    // High-frequency arpeggiator notes (A5, C6, E6, G6, A6, C7, E7, G7)
    const ARP_NOTES = [
      880.0, 1046.5, 1318.51, 1567.98, 1760.0, 2093.0, 2637.02, 3135.96,
    ];

    // Song arrangement routing tables
    const kickPatternIds = new Uint8Array(numMeasures);
    const hatPatternIds = new Uint8Array(numMeasures);
    const snareModes = new Uint8Array(numMeasures); // 0=silent, 1=active (on beats 2 & 4)
    const rideModes = new Uint8Array(numMeasures); // 0=silent, 1=off-beat cymbals
    const bassModes = new Uint8Array(numMeasures); // 0=silent, 1=drone, 2=rolling
    const melodyIds = new Uint8Array(numMeasures); // 0=silent, 1=intro, 2=main, 3=breakdown, 4=climax
    const arpModes = new Uint8Array(numMeasures); // 0=silent, 1=8th-note arp, 2=16th-note fast arp
    const padModes = new Uint8Array(numMeasures); // 0=silent, 1=soft, 2=full
    const cutoffCut = new Float32Array(numMeasures); // Filter cutoff: 0.0 - 1.0

    const fillRange = (arr, startPct, endPct, val) => {
      const start = Math.floor(numMeasures * startPct);
      const end = Math.floor(numMeasures * endPct);
      for (let m = start; m <= end && m < numMeasures; m++) {
        arr[m] = val;
      }
    };

    // Default configuration
    kickPatternIds.fill(0);
    hatPatternIds.fill(0);
    snareModes.fill(0);
    rideModes.fill(0);
    bassModes.fill(0);
    melodyIds.fill(0);
    arpModes.fill(0);
    padModes.fill(0);
    cutoffCut.fill(0.5);

    // 1. Intro (0% - 15%)
    const introEndPct = 0.15;
    fillRange(bassModes, 0, introEndPct, 1); // drone bass
    fillRange(padModes, 0, introEndPct, 1); // soft chords (mid-freq)
    fillRange(cutoffCut, 0, introEndPct, 0.2); // dark filter

    const kickStartPct = 0.15 * 0.3;
    const hatStartPct = 0.15 * 0.75;
    fillRange(kickPatternIds, kickStartPct, introEndPct, 1); // standard kick
    fillRange(bassModes, kickStartPct, introEndPct, 2); // rolling bass
    fillRange(melodyIds, kickStartPct, introEndPct, 1); // intro melody
    fillRange(hatPatternIds, hatStartPct, introEndPct, 1); // offbeat hats
    fillRange(cutoffCut, kickStartPct, introEndPct, 0.35);

    // 2. Main Groove (15% - 40%)
    fillRange(kickPatternIds, 0.15, 0.4, 1);
    fillRange(hatPatternIds, 0.15, 0.4, 1);
    fillRange(snareModes, 0.15, 0.4, 1); // snare enters
    fillRange(bassModes, 0.15, 0.4, 2);
    fillRange(padModes, 0.15, 0.4, 1);
    fillRange(melodyIds, 0.15, 0.25, 1);
    fillRange(melodyIds, 0.25, 0.4, 2); // main melody
    fillRange(arpModes, 0.25, 0.4, 1); // sparkling arp starts (high-freq)
    fillRange(cutoffCut, 0.15, 0.4, 0.55);

    // 3. Development / Energy Rise (40% - 55%)
    fillRange(kickPatternIds, 0.4, 0.55, 2); // accented kick
    fillRange(hatPatternIds, 0.4, 0.55, 2); // driving hats
    fillRange(snareModes, 0.4, 0.55, 1);
    fillRange(bassModes, 0.4, 0.55, 2);
    fillRange(padModes, 0.4, 0.55, 1);
    fillRange(melodyIds, 0.4, 0.55, 2);
    fillRange(arpModes, 0.4, 0.55, 2); // faster arpeggiator
    fillRange(cutoffCut, 0.4, 0.55, 0.7);

    // 4. Breakdown (55% - 70%)
    const breakdownStart = Math.floor(numMeasures * 0.55);
    const breakdownEnd = Math.floor(numMeasures * 0.7);
    fillRange(kickPatternIds, 0.55, 0.7, 0); // silence kick
    fillRange(hatPatternIds, 0.55, 0.7, 0); // silence hats
    fillRange(snareModes, 0.55, 0.7, 0);
    fillRange(bassModes, 0.55, 0.7, 1); // drone bass only
    fillRange(padModes, 0.55, 0.7, 2); // full chords (rich mids)
    fillRange(melodyIds, 0.55, 0.7, 3); // breakdown melody
    fillRange(arpModes, 0.55, 0.7, 1); // gentle high arp
    fillRange(cutoffCut, 0.55, 0.7, 0.45);

    // 5. Build-up (70% - 77%)
    const buildupStart = Math.floor(numMeasures * 0.7);
    const buildupEnd = Math.floor(numMeasures * 0.77);
    fillRange(kickPatternIds, 0.7, 0.77, 3); // kick roll
    fillRange(hatPatternIds, 0.7, 0.77, 1);
    fillRange(snareModes, 0.7, 0.77, 1);
    fillRange(bassModes, 0.7, 0.77, 0); // bass drop
    fillRange(padModes, 0.7, 0.77, 1);
    fillRange(melodyIds, 0.7, 0.77, 2);
    fillRange(arpModes, 0.7, 0.77, 2); // build-up fast arp

    // 6. Drop / Climax (77% - 92%)
    fillRange(kickPatternIds, 0.77, 0.92, 2); // busy kick
    fillRange(hatPatternIds, 0.77, 0.92, 3); // climax hats
    fillRange(snareModes, 0.77, 0.92, 1);
    fillRange(rideModes, 0.77, 0.92, 1); // metallic cymbals enter
    fillRange(bassModes, 0.77, 0.92, 2);
    fillRange(padModes, 0.77, 0.92, 2); // full chords
    fillRange(melodyIds, 0.77, 0.92, 4); // climax melody
    fillRange(arpModes, 0.77, 0.92, 2); // fast arp
    fillRange(cutoffCut, 0.77, 0.92, 0.85);

    // 7. Outro & Fade (92% - 100%)
    fillRange(kickPatternIds, 0.92, 0.97, 1);
    fillRange(hatPatternIds, 0.92, 0.97, 1);
    fillRange(snareModes, 0.92, 0.97, 0);
    fillRange(bassModes, 0.92, 0.97, 2);
    fillRange(padModes, 0.92, 0.97, 1);
    fillRange(melodyIds, 0.92, 0.97, 1);
    fillRange(arpModes, 0.92, 0.97, 0);
    fillRange(cutoffCut, 0.92, 0.97, 0.45);

    fillRange(kickPatternIds, 0.97, 1.0, 1);
    fillRange(hatPatternIds, 0.97, 1.0, 0);
    fillRange(bassModes, 0.97, 1.0, 1);
    fillRange(padModes, 0.97, 1.0, 1);
    fillRange(melodyIds, 0.97, 1.0, 0);
    fillRange(cutoffCut, 0.97, 1.0, 0.2);

    // Sequencer runtime states
    let lastStepIndex = -1;
    let lastKickTime = -999.0;
    let lastHatTime = -999.0;
    let lastSnareTime = -999.0;
    let lastRideTime = -999.0;
    let lastBassTime = -999.0;
    let lastBassFreq = 55.0;
    let lastSynthTime = -999.0;
    let lastSynthFreq = 110.0;
    let lastArpTime = -999.0;
    let lastArpFreq = 1046.5;

    // Pitch portamento states
    let activeSynthFreq = 110.0;
    let activeArpFreq = 1046.5;

    // Chamberlin SVF state variables (for high Q and zero frequency clipping)
    let svfSynthL = 0.0,
      svfSynthB = 0.0;
    let svfPadL = 0.0,
      svfPadB = 0.0;
    let svfArpL = 0.0,
      svfArpB = 0.0;
    let svfRiserL = 0.0,
      svfRiserB = 0.0;

    let lpBassState = 0.0;

    for (let i = 0; i < totalSamples; i++) {
      const t = i / sampleRate;
      const stepIndex = Math.floor(t / quarterBeat);
      const measureIndex = Math.min(
        Math.floor(stepIndex / stepsPerMeasure),
        numMeasures - 1,
      );
      const stepInMeasure = stepIndex % stepsPerMeasure;

      // Trigger events on step boundaries
      if (stepIndex !== lastStepIndex) {
        lastStepIndex = stepIndex;

        // 1. Kick Trigger
        const activeKickPattern = kickPatternIds[measureIndex];
        if (activeKickPattern > 0) {
          const kickPattern =
            activeKickPattern === 1
              ? PATTERN_KICK_STANDARD
              : activeKickPattern === 2
                ? PATTERN_KICK_ACCENTED
                : PATTERN_KICK_ROLL;
          const playKick = (kickPattern & (1 << (15 - stepInMeasure))) !== 0;
          if (playKick) {
            lastKickTime = t;
          }
        }

        // 2. Hat Trigger
        const activeHatPattern = hatPatternIds[measureIndex];
        if (activeHatPattern > 0) {
          const hatPattern =
            activeHatPattern === 1
              ? PATTERN_HAT_OFFBEAT
              : activeHatPattern === 2
                ? PATTERN_HAT_DRIVING
                : PATTERN_HAT_CLIMAX;
          const playHat = (hatPattern & (1 << (15 - stepInMeasure))) !== 0;
          if (playHat) {
            lastHatTime = t;
          }
        }

        // 3. Snare Trigger
        const activeSnareMode = snareModes[measureIndex];
        if (activeSnareMode > 0) {
          const playSnare = stepInMeasure === 4 || stepInMeasure === 12;
          if (playSnare) {
            lastSnareTime = t;
          }
        }

        // 4. Ride Cymbal Trigger
        const activeRideMode = rideModes[measureIndex];
        if (activeRideMode > 0) {
          const playRide = stepInMeasure % 4 === 2; // offbeats
          if (playRide) {
            lastRideTime = t;
          }
        }

        // 5. Bass Trigger
        const activeBassMode = bassModes[measureIndex];
        if (activeBassMode === 2) {
          const bassPattern = 0b0111011101110111;
          const playBass = (bassPattern & (1 << (15 - stepInMeasure))) !== 0;
          if (playBass) {
            lastBassTime = t;
            const bassPitches = [
              55.0, 55.0, 65.41, 65.41, 49.0, 49.0, 43.65, 43.65,
            ];
            const bassPitchIdx =
              Math.floor(measureIndex / 2) % bassPitches.length;
            lastBassFreq = bassPitches[bassPitchIdx];
          }
        } else if (activeBassMode === 1) {
          if (stepInMeasure === 0) {
            lastBassTime = t;
            const bassPitches = [
              55.0, 55.0, 65.41, 65.41, 49.0, 49.0, 43.65, 43.65,
            ];
            const bassPitchIdx =
              Math.floor(measureIndex / 2) % bassPitches.length;
            lastBassFreq = bassPitches[bassPitchIdx];
          }
        }

        // 6. Synth Trigger
        const activeMelodyId = melodyIds[measureIndex];
        if (activeMelodyId > 0) {
          let melody;
          if (activeMelodyId === 1) melody = MELODIES.intro;
          else if (activeMelodyId === 2) melody = MELODIES.main;
          else if (activeMelodyId === 3) melody = MELODIES.breakdown;
          else melody = MELODIES.climax;

          const freqSynth = melody[stepInMeasure];
          if (freqSynth > 0) {
            lastSynthTime = t;
            lastSynthFreq = freqSynth;
          }
        }

        // 7. Arpeggiator Trigger
        const activeArpMode = arpModes[measureIndex];
        if (activeArpMode > 0) {
          const playArp = activeArpMode === 1 ? stepInMeasure % 2 === 1 : true;
          if (playArp) {
            lastArpTime = t;
            const noteIdx =
              (stepInMeasure + measureIndex * 5) % ARP_NOTES.length;
            lastArpFreq = ARP_NOTES[noteIdx];
          }
        }
      }

      // Apply portamento (pitch glide) to active frequencies (performance-oriented linear blend)
      const glideAlpha = 0.0025;
      activeSynthFreq += glideAlpha * (lastSynthFreq - activeSynthFreq);
      activeArpFreq += glideAlpha * (lastArpFreq - activeArpFreq);

      // --- Signal Synthesis ---

      // 1. Kick Drum (Low Frequencies - Red/Orange)
      let kick = 0.0;
      if (lastKickTime > -900.0) {
        const dtKick = t - lastKickTime;
        if (dtKick < 0.4) {
          const freqKick = 42.0 + 160.0 * Math.exp(-dtKick * 45.0);
          const kickEnv = Math.exp(-dtKick * 12.0);
          const rawKick =
            Math.sin(2.0 * Math.PI * freqKick * dtKick) * kickEnv * 0.85;
          kick = Math.tanh(rawKick * 1.8);
        }
      }

      // 2. Open / Closed Hi-hat (Very High Frequencies - Blue/Violet)
      let hat = 0.0;
      if (lastHatTime > -900.0) {
        const dtHat = t - lastHatTime;
        if (dtHat < 0.25) {
          const activeHatPattern = hatPatternIds[measureIndex];
          const hatDecay =
            activeHatPattern === 3 && stepInMeasure % 4 === 2 ? 6.0 : 16.0;
          const hatEnv = Math.exp(-dtHat * hatDecay);
          hat = (Math.random() * 2.0 - 1.0) * hatEnv * 0.12;
        }
      }

      // 3. Snare Drum (Mid-High Frequencies - Green/Blue)
      let snare = 0.0;
      if (lastSnareTime > -900.0) {
        const dtSnare = t - lastSnareTime;
        if (dtSnare < 0.3) {
          const toneEnv = Math.exp(-dtSnare * 30.0);
          const toneFreq = 180.0 * Math.exp(-dtSnare * 20.0);
          const tone =
            Math.sin(2.0 * Math.PI * toneFreq * dtSnare) * toneEnv * 0.3;

          const noiseEnv = Math.exp(-dtSnare * 14.0);
          const noise = (Math.random() * 2.0 - 1.0) * noiseEnv * 0.45;
          snare = (tone + noise) * 0.22;
        }
      }

      // 4. Sub & Rolling Bassline (Low Frequencies - Red/Orange)
      let bass = 0.0;
      const activeBassMode = bassModes[measureIndex];
      if (activeBassMode > 0 && lastBassTime > -900.0) {
        const dtBass = t - lastBassTime;
        const bassEnv = activeBassMode === 1 ? 1.0 : Math.exp(-dtBass * 16.0);

        const phaseBass = t * lastBassFreq;
        const sawBass = 2.0 * (phaseBass - Math.floor(phaseBass + 0.5));

        const lpBassAlpha = 0.18;
        lpBassState = lpBassState + lpBassAlpha * (sawBass - lpBassState);

        let ducking = 1.0;
        if (lastKickTime > -900.0) {
          const dtKick = t - lastKickTime;
          if (dtKick < 0.18) {
            ducking = 1.0 - Math.exp(-dtKick * 7.0) * 0.85;
          }
        }

        bass = lpBassState * bassEnv * ducking * 0.35;
      }

      // 5. Lead Synth (Mid Frequencies with Resonant SVF Filter and LFO Glide)
      let synth = 0.0;
      const activeMelodyId = melodyIds[measureIndex];
      if (activeMelodyId > 0 && lastSynthTime > -900.0) {
        const dtSynth = t - lastSynthTime;
        if (dtSynth < 0.5) {
          const phaseSynth = t * activeSynthFreq;
          const sqSynth = Math.sign(Math.sin(2.0 * Math.PI * phaseSynth));
          const sawSynth = 2.0 * (phaseSynth - Math.floor(phaseSynth + 0.5));
          const rawSynth = sqSynth * 0.5 + sawSynth * 0.5;

          const decaySynth = activeMelodyId === 3 ? 5.0 : 8.0;
          const envSynth = Math.exp(-dtSynth * decaySynth);

          // Unsynchronized LFO 0.2Hz sweep on Lead Synth Cutoff
          let cutoffFreq =
            300.0 + 1800.0 * (0.5 + 0.5 * Math.sin(2.0 * Math.PI * 0.2 * t));

          if (measureIndex >= buildupStart && measureIndex <= buildupEnd) {
            const buildupMeasures = buildupEnd - buildupStart + 1;
            const currentOffset = measureIndex - buildupStart;
            const measureProgress = (t % measureLen) / measureLen;
            const totalProgress =
              (currentOffset + measureProgress) / buildupMeasures;
            cutoffFreq = 200.0 + 3800.0 * totalProgress;
          } else if (activeMelodyId === 3) {
            cutoffFreq =
              350.0 + 1000.0 * (0.5 + 0.5 * Math.sin(2.0 * Math.PI * 0.15 * t));
          }

          const fSynth = Math.min(
            0.95,
            2.0 * Math.sin((Math.PI * cutoffFreq) / sampleRate),
          );
          const qSynth = 0.45; // resonant damping
          const highSynth = rawSynth - svfSynthL - qSynth * svfSynthB;
          svfSynthB += fSynth * highSynth;
          svfSynthL += fSynth * svfSynthB;

          synth = svfSynthL * envSynth * 0.085;
        }
      }

      // 6. Chord Pad (Mid Frequencies - Resonant SVF Lowpass + slow 0.08Hz LFO)
      let pad = 0.0;
      const activePadMode = padModes[measureIndex];
      if (activePadMode > 0) {
        const chordIdx = Math.floor(measureIndex / 2) % CHORDS.length;
        const activeChord = CHORDS[chordIdx];
        const osc1 = Math.sin(2.0 * Math.PI * activeChord[0] * t);
        const osc2 = Math.sin(2.0 * Math.PI * activeChord[1] * t);
        const osc3 = Math.sin(2.0 * Math.PI * activeChord[2] * t);
        const rawPad = (osc1 + osc2 + osc3) / 3.0;

        const padCutoff =
          150.0 + 1200.0 * (0.5 + 0.5 * Math.cos(2.0 * Math.PI * 0.08 * t));
        const fPad = Math.min(
          0.95,
          2.0 * Math.sin((Math.PI * padCutoff) / sampleRate),
        );
        const qPad = 0.5; // low damping
        const highPad = rawPad - svfPadL - qPad * svfPadB;
        svfPadB += fPad * highPad;
        svfPadL += fPad * svfPadB;

        let ducking = 1.0;
        if (lastKickTime > -900.0) {
          const dtKick = t - lastKickTime;
          if (dtKick < 0.2) {
            ducking = 1.0 - Math.exp(-dtKick * 6.0) * 0.75;
          }
        }

        const padVol = activePadMode === 1 ? 0.11 : 0.18;
        pad = svfPadL * padVol * ducking;
      }

      // 7. High-Frequency Arpeggiator (High Frequencies - Resonant SVF Bandpass + LFO)
      let arp = 0.0;
      if (lastArpTime > -900.0) {
        const dtArp = t - lastArpTime;
        if (dtArp < 0.2) {
          const phaseArp = t * activeArpFreq;
          const triArp =
            1.0 - 4.0 * Math.abs(Math.round(phaseArp - 0.5) - (phaseArp - 0.5));
          const sqArp = Math.sign(Math.sin(2.0 * Math.PI * phaseArp));
          const rawArp = triArp * 0.75 + sqArp * 0.25;

          const envArp = Math.exp(-dtArp * 20.0);

          const arpCutoff =
            800.0 + 2200.0 * (0.5 + 0.5 * Math.sin(2.0 * Math.PI * 0.35 * t));
          const fArp = Math.min(
            0.95,
            2.0 * Math.sin((Math.PI * arpCutoff) / sampleRate),
          );
          const qArp = 0.22; // narrow bandpass
          const highArp = rawArp - svfArpL - qArp * svfArpB;
          svfArpB += fArp * highArp;
          svfArpL += fArp * svfArpB;

          arp = svfArpB * envArp * 0.06; // Bandpass output!
        }
      }

      // 8. Metallic Ride Cymbal (Very High Frequencies - Blue/Violet)
      let ride = 0.0;
      if (lastRideTime > -900.0) {
        const dtRide = t - lastRideTime;
        if (dtRide < 0.45) {
          const metallic =
            Math.sin(2.0 * Math.PI * 2500.0 * t) * 0.3 +
            Math.sin(2.0 * Math.PI * 3800.0 * t) * 0.3 +
            Math.sin(2.0 * Math.PI * 5200.0 * t) * 0.2 +
            (Math.random() * 2.0 - 1.0) * 0.2;
          const envRide = Math.exp(-dtRide * 7.5);
          ride = metallic * envRide * 0.08;
        }
      }

      // 9. White Noise Riser (Resonant SVF Highpass Filter Sweep)
      let riser = 0.0;
      if (measureIndex >= breakdownStart && measureIndex <= buildupEnd) {
        const totalMeasures = buildupEnd - breakdownStart + 1;
        const currentOffset = measureIndex - breakdownStart;
        const measureProgress = (t % measureLen) / measureLen;
        const progress = (currentOffset + measureProgress) / totalMeasures;

        const riserVol = Math.pow(progress, 2.5) * 0.12;
        const rawNoise = Math.random() * 2.0 - 1.0;

        const riserCutoff = 150.0 + 3650.0 * Math.pow(progress, 2.0);
        const fRiser = Math.min(
          0.95,
          2.0 * Math.sin((Math.PI * riserCutoff) / sampleRate),
        );
        const qRiser = 0.3; // resonant Q

        const highRiser = rawNoise - svfRiserL - qRiser * svfRiserB;
        svfRiserB += fRiser * highRiser;
        svfRiserL += fRiser * svfRiserB;

        riser = highRiser * riserVol; // Highpass output!
      }

      const rawMix =
        kick * 0.42 +
        hat * 0.18 +
        snare * 0.18 +
        ride * 0.18 +
        bass * 0.26 +
        synth * 0.18 +
        pad * 0.2 +
        arp * 0.16 +
        riser * 0.16;
      pcm[i] = Math.tanh(rawMix * 1.15) * 0.95;
    }
  } else if (type === "Ambient") {
    for (let i = 0; i < totalSamples; i++) {
      const t = i / sampleRate;
      const bass =
        (Math.sin(2 * Math.PI * 65 * t) + Math.sin(2 * Math.PI * 98 * t)) * 0.3;
      const lfo1 = 0.5 + 0.5 * Math.sin(2 * Math.PI * 0.15 * t);
      const mids =
        (Math.sin(2 * Math.PI * 261 * t) +
          Math.sin(2 * Math.PI * 329 * t) +
          Math.sin(2 * Math.PI * 392 * t)) *
        lfo1 *
        0.2;
      const lfo2 = 0.5 + 0.5 * Math.cos(2 * Math.PI * 0.4 * t);
      const highs = Math.sin(2 * Math.PI * 2500 * t) * lfo2 * 0.08;
      pcm[i] = (bass + mids + highs) * 0.9;
    }
  } else if (type === "Sweep") {
    const f0 = 20;
    const f1 = 5000;
    const r = Math.log(f1 / f0) / duration;
    for (let i = 0; i < totalSamples; i++) {
      const t = i / sampleRate;
      const freq = f0 * Math.exp(r * t);
      pcm[i] = Math.sin(2 * Math.PI * freq * t) * 0.5;
    }
  }

  self.postMessage(pcm, [pcm.buffer]);
};
