/** Dictionnaire français (langue par défaut / repli). */
export const FR: Record<string, string> = {
  // Commun ---------------------------------------------------------------
  'common.yes': 'Oui',
  'common.no': 'Non',
  'common.required': 'Valeur requise.',
  'common.close': 'Fermer',
  'common.choose': 'Choisir...',
  'common.empty': '-',

  // Portail public — chrome ---------------------------------------------
  'public.brand.subtitle': 'Portail public',
  'public.lang.aria': 'Langue',
  'public.nav.agent': 'Agent',
  'public.nav.pv': 'PV',
  'public.nav.report': 'Signaler',
  'public.nav.tracking': 'Suivi',
  'public.nav.about': 'À propos',

  // Pourquoi (remarque 12) ----------------------------------------------
  'public.why.agent':
    "Pourquoi vérifier l'identité d'un agent ? Assurez-vous qu'une personne se présentant comme agent APM est bien enregistrée dans le système.",
  'public.why.pv':
    "Pourquoi suivre un PV ? Vérifiez l'existence et le statut d'un procès-verbal qui vous a été remis.",
  'public.why.report':
    "Pourquoi signaler ? Informez les autorités municipales d'un incident constaté dans votre quartier.",

  // Contact / infoline (remarques PP-02, 14bis) --------------------------
  'public.contact.title': "Besoin d'aide ?",
  'public.contact.infoline': 'Infoline',
  'public.contact.whatsapp': 'WhatsApp',

  // Vérification agent / PV ---------------------------------------------
  'public.verify.eyebrow': 'Vérification publique',
  'public.verify.agent.title': 'Vérifier un agent',
  'public.verify.pv.title': 'Vérifier un PV',
  'public.verify.subtitle':
    'Les données affichées sont limitées aux informations utiles à la vérification publique.',
  'public.verify.agent.label': 'Matricule agent',
  'public.verify.pv.label': 'Numéro PV',
  'public.verify.submit': 'Vérifier',
  'public.verify.agent.notFound':
    'Le matricule saisi ne trouve aucune correspondance dans la base de données.',
  'public.verify.pv.notFound':
    'Le numéro de PV saisi ne trouve aucune correspondance dans la base de données.',
  'public.verify.agent.photoAlt': "Photo de l'agent",
  'public.verify.agent.help':
    "Saisissez le matricule figurant sur la carte professionnelle de l'agent pour confirmer son identité.",
  'public.verify.pv.help':
    'Saisissez le numéro du procès-verbal (PV-…) pour vérifier son existence et son statut.',

  // Signalement ----------------------------------------------------------
  'public.report.eyebrow': 'Signalement citoyen',
  'public.report.title': 'Déposer un signalement',
  'public.report.subtitle': 'Un numéro de suivi est généré après validation.',
  'public.report.help':
    'Un signalement est une plainte contre un agent. Indiquez la localisation (région, département, commune), le type d’action contestée et l’agent concerné, puis décrivez les faits. Le contact est facultatif.',
  'public.report.region': 'Région',
  'public.report.departement': 'Département',
  'public.report.commune': 'Commune',
  'public.report.category': 'Catégorie de signalement',
  'public.report.type': 'Type d’action contestée',
  'public.report.type.amende': 'Amende',
  'public.report.type.verbalisation': 'Verbalisation',
  'public.report.type.scelle': 'Mise sous scellé',
  'public.report.type.fourriere': 'Mise en fourrière',
  'public.report.type.autre': 'Autre',
  'public.report.reportedAgentMat': 'Matricule de l’agent visé',
  'public.report.reportedAgentName': 'Nom de l’agent visé',
  'public.report.incidentDate': 'Date et heure de l’incident',
  'public.report.pvRef': 'Numéro de PV concerné',
  'public.report.zone': 'Quartier / zone',
  'public.report.lieuDit': 'Lieu-dit',
  'public.report.description': 'Description',
  'public.report.anonymous': 'Rester anonyme',
  'public.report.contactName': 'Nom du contact',
  'public.report.contactPhone': 'Téléphone / WhatsApp',
  'public.report.submit': 'Envoyer',
  'public.report.submitting': 'Envoi...',
  'public.report.newReport': 'Nouveau signalement',
  'public.report.tracking': 'Numéro de suivi : {value}',
  'public.report.successNote':
    'Conservez ce numéro pour suivre votre signalement. Si vous avez fourni un contact, vous pourriez être recontacté par WhatsApp.',
  'public.report.error': 'Signalement refusé. Vérifiez les informations saisies.',
  'public.report.communesError': 'Impossible de charger les communes disponibles.',
  'public.report.optionsError': 'Options indisponibles pour cette commune.',
  'public.report.geoError': 'Impossible de charger la géographie disponible.',

  // Suivi signalement ----------------------------------------------------
  'public.track.eyebrow': 'Suivi public',
  'public.track.title': 'Suivre un signalement',
  'public.track.subtitle':
    "Le suivi public n'expose pas les notes administratives ni les contacts.",
  'public.track.label': 'Numéro de suivi',
  'public.track.submit': 'Consulter',
  'public.track.required': 'Numéro requis.',
  'public.track.notFound':
    'Le code saisi ne correspond pas à un numéro de signalement dans la base de données.',
  'public.track.help':
    'Saisissez le numéro de suivi (SIG-…) reçu lors du dépôt pour consulter l’avancement.',

  // À propos (remarque PP-04) -------------------------------------------
  'public.about.eyebrow': 'À propos',
  'public.about.title': "À propos d'APMTRACK",
  'public.about.intro':
    "APMTRACK est la plateforme de gestion des Agents de Police Municipale (APM). Elle outille les communes pour le suivi des agents de terrain, l'émission et le paiement des procès-verbaux, et le traitement des signalements citoyens.",
  'public.about.missionTitle': 'Notre mission',
  'public.about.missionBody':
    "Renforcer la transparence et l'efficacité de la police municipale en offrant aux citoyens des outils simples : vérifier l'identité d'un agent, contrôler un procès-verbal et signaler un incident.",
  'public.about.rolesTitle': 'Rôles et missions',
  'public.about.rolesBody':
    "Les agents APM veillent au respect des arrêtés municipaux, à la salubrité et à l'ordre public local. Chaque action est tracée et rattachée à une commune.",

  // Libellés de champs publics ------------------------------------------
  'field.matricule': 'Matricule',
  'field.agent_matricule': 'Dressé par',
  'field.full_name': 'Nom',
  'field.status': 'Statut',
  'field.active': 'En activité',
  'field.commune_nom': 'Commune',
  'field.pv_number': 'Numéro PV',
  'field.amount_initial_fcfa': 'Montant',
  'field.signalement_number': 'Numéro de suivi',
  'field.type_incident': "Type d'incident",
  'field.created_at': 'Créé le',
  'field.updated_at': 'Mis à jour le',
};
