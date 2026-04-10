<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00325" is-a="MutuallyExclusiveAttributeValues">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00325][Error] If ISM_USGOV_RESOURCE, then tokens [OC] 
        and [RELIDO] are mutually exclusive for attribute @ism:disseminationControls.
    </sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		This rule uses an abstract pattern to consolidate logic.
		If the document is an ISM_USGOV_RESOURCE, for each element which 
		has attribute @ism:disseminationControls specified with a value 
		containing the token [OC] or [RELIDO], this rule ensures that attribute
		@ism:disseminationControls is specified with a value containing only 
		one of the tokens [OC] or [RELIDO].
	</sch:p>
	<sch:param name="context" value="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC', 'RELIDO'))]"/>
	<sch:param name="attrValue" value="@ism:disseminationControls"/>
	<sch:param name="mutuallyExclusiveTokenList" value="('OC', 'RELIDO')"/>
	<sch:param name="errMsg" value="'   [ISM-ID-00325][Error] If ISM_USGOV_RESOURCE, then tokens [OC]    and [RELIDO] are mutually exclusive for attribute disseminationControls.   '"/>
</sch:pattern>