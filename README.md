# GuLoader - Professional Manga Monitoring System

🚀 **Автоматическая система мониторинга и загрузки манги с поддержкой GitHub Actions**

## ✨ Возможности

- 🔍 **Автоматическое сканирование** манги каждые 5 минут
- 📥 **Автоматическая загрузка** новых глав
- 🔄 **Повторные попытки** для платных глав (когда они станут бесплатными)
- 🗄️ **База данных SQLite** для отслеживания состояния
- 📦 **ZIP архивы** для каждой главы
- 🌐 **Поддержка множества сайтов**: Eros Moon, Madarascans, Thunderscans
- 🤖 **GitHub Actions** для полной автоматизации

## 🛠️ Установка и настройка

### 1. Клонирование репозитория

```bash
git clone <your-repo-url>
cd guloader
```

### 2. Настройка конфигурации

Отредактируйте файл `config.toml`:

```toml
[database]
url = "sqlite:data/guloader.db?mode=rwc"

[storage]
base_path = "./downloads"
scans_path = "./scans"
max_size_gb = 50
compression = true
thumbnail_size = 200

# Добавьте свои манги
[[manga]]
title = "Название манги"
site = "eros"  # или "madara", "thunder"
url = "https://eros-moon.xyz/manga/your-manga/"
```

### 3. Инициализация базы данных

```bash
cargo build --release
./target/release/guloader init
```

### 4. Настройка GitHub Actions

GitHub Actions уже настроен в `.github/workflows/manga-monitor.yml` и будет:

- ✅ Запускаться каждые 5 минут
- ✅ Сканировать все манги из `config.toml`
- ✅ Автоматически загружать новые главы
- ✅ Коммитить скачанные файлы в репозиторий
- ✅ Повторять попытки для неудачных загрузок

### 5. Активация GitHub Actions

1. Перейдите в **Settings** → **Actions** → **General**
2. Включите **Actions** для репозитория
3. GitHub Actions автоматически начнет работать

## 📋 Команды CLI

### Основные команды

```bash
# Инициализация базы данных
./guloader init

# Мониторинг манги (автоматическая загрузка новых глав)
./guloader monitor

# Сканирование всех манги
./guloader scan

# Сканирование только новых глав
./guloader scan --new

# Сканирование конкретной манги
./guloader scan "Название манги"

# Загрузка конкретной главы
./guloader download eros https://eros-moon.xyz/manga/chapter-123/

# Показать статус
./guloader status

# Очистка старых загрузок
./guloader cleanup --days 30
```

### Поддерживаемые сайты

- **`eros`** - Eros Moon (eros-moon.xyz)
- **`madara`** - Madarascans (madarascans.com)  
- **`thunder`** - Thunderscans (en-thunderscans.com)

## 📁 Структура файлов

```
guloader/
├── .github/workflows/
│   └── manga-monitor.yml    # GitHub Actions workflow
├── downloads/               # Ручные загрузки
│   └── <manga_title>/
│       └── <chapter_number>/
│           ├── pages/        # Изображения страниц
│           └── Chapter_X.zip # ZIP архив
├── scans/                   # Автоматические загрузки
│   └── <manga_title>/
│       └── <chapter_number>/
│           ├── pages/        # Изображения страниц
│           └── Chapter_X.zip # ZIP архив
├── data/
│   └── guloader.db         # База данных SQLite
├── config.toml             # Конфигурация
└── target/release/guloader # Исполняемый файл
```

## 🔧 Настройка GitHub Actions

### Автоматический запуск

GitHub Actions настроен для автоматического запуска:

- **По расписанию**: каждые 5 минут (`*/5 * * * *`)
- **При push**: в main ветку
- **При изменении**: файла `config.toml`
- **Вручную**: через GitHub UI

### Мониторинг

Проверьте статус выполнения в **Actions** вкладке вашего репозитория.

### Логи

Все логи сохраняются в GitHub Actions. При ошибках создается артефакт с логами.

## 🚀 Производительность

- ⚡ **Быстрое сканирование**: ~1-2 секунды на мангу
- 🔄 **Параллельная загрузка**: все изображения загружаются одновременно
- 💾 **Эффективное хранение**: ZIP архивы для экономии места
- 🗄️ **Оптимизированная БД**: SQLite с индексами

## 🛡️ Надежность

- 🔄 **Повторные попытки**: автоматические повторы для неудачных загрузок
- 📊 **Отслеживание состояния**: полная история в базе данных
- 🚫 **Защита от дублирования**: проверка существующих глав
- ⚠️ **Обработка ошибок**: graceful handling всех ошибок

## 📈 Мониторинг

### Статистика в логах

```
📊 Monitoring cycle completed:
   🆕 New chapters found: 5
   ⬇️ Chapters downloaded: 3
   ❌ Failed downloads: 2
```

### Статус манги

```bash
./guloader status
```

Показывает:
- Количество глав в базе данных
- Последнее обновление
- Статус каждой манги

## 🔧 Расширение

### Добавление новых сайтов

1. Создайте новый сканер в `src/scanners/`
2. Создайте новый загрузчик в `src/downloaders/`
3. Зарегистрируйте их в `src/registry.rs`
4. Добавьте конфигурацию в `config.toml`

### Кастомизация расписания

Отредактируйте `.github/workflows/manga-monitor.yml`:

```yaml
schedule:
  # Каждые 10 минут
  - cron: '*/10 * * * *'
  # Каждый час
  - cron: '0 * * * *'
```

## 🆘 Устранение неполадок

### GitHub Actions не запускается

1. Проверьте, что Actions включены в настройках репозитория
2. Убедитесь, что файл `.github/workflows/manga-monitor.yml` существует
3. Проверьте права доступа к репозиторию

### Ошибки загрузки

1. Проверьте URL манги в `config.toml`
2. Убедитесь, что сайт доступен
3. Проверьте логи в GitHub Actions

### Проблемы с базой данных

```bash
# Пересоздать базу данных
rm data/guloader.db
./guloader init
```

## 📝 Лицензия

MIT License - используйте свободно для личных и коммерческих проектов.

## 🤝 Вклад в проект

Приветствуются pull requests для:
- Новых сайтов манги
- Улучшений производительности
- Исправления багов
- Документации

---

**🎊 Наслаждайтесь автоматической загрузкой манги!**