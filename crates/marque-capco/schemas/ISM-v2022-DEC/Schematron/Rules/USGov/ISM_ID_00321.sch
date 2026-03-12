<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00321" is-a="MutuallyExclusiveAttributeValues">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00321][Error] If ISM_USGOV_RESOURCE, then tokens [RD], [FRD] and
		[TFNI] are mutually exclusive for attribute @ism:atomicEnergyMarkings. 
		
		Human Readable: RD, FRD, and TFNI are mutually exclusive and cannot be commingled in a portion mark or in the banner line. 
	</sch:p>
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		This rule uses an abstract pattern to consolidate logic. If the document
		is an ISM_USGOV_RESOURCE, for each element which has attribute @ism:atomicEnergyMarkings
		specified with a value containing the token [RD], [FRD] or [TFNI], this rule ensures that attribute
		@ism:disseminationControls is specified with a value containing only one of the tokens [RD],
		[FRD] or [TFNI].
	</sch:p>
	<sch:param name="context" value="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD', 'FRD', 'TFNI'))]"/>
	<sch:param name="attrValue" value="@ism:atomicEnergyMarkings"/>
	<sch:param name="mutuallyExclusiveTokenList" value="('RD', 'FRD', 'TFNI')"/>
	<sch:param name="errMsg" value="'         [ISM-ID-00321][Error] If ISM_USGOV_RESOURCE, then tokens [RD],                [FRD] and [TFNI] are mutually exclusive for attribute atomicEnergyMarkings.         Human Readable: RD, FRD and TFNI are mutually exclusive and cannot be commingled         in a portion mark or in the banner line.         '"/>
</sch:pattern>