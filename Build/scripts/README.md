# Buildscripts

## Setup

```bash
cd Build/scripts
python -m pip install -r requirements.txt
```

## Usage

```bash
# Global quality gates
python saikuro_build.py quality

# Single adapter
python saikuro_build.py adapter rust
python saikuro_build.py adapter c
python saikuro_build.py adapter cpp
python saikuro_build.py adapter typescript
python saikuro_build.py adapter python
python saikuro_build.py adapter csharp

# Run everything
python saikuro_build.py all
```
