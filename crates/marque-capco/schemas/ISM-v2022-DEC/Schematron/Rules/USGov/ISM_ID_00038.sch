<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00038" is-a="MutuallyExclusiveAttributeValues">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00038][Error] If ISM_USGOV_RESOURCE, then the tokens [XD], [ND],
		[SBU], and [SBU-NF] are mutually exclusive for attribute @ism:nonICmarkings. 
		
		Human Readable: USA documents must not specify [XD], [ND], [SBU], and/or [SBU-NF] commingled on a single element. 
	</sch:p>
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		This rule uses an abstract pattern to consolidate logic. If the document
		is an ISM_USGOV_RESOURCE, for each element which has attribute @ism:nonICmarkings specified with
		a value containing the token [XD], [ND], [SBU], or [SBU-NF] this rule ensures that attribute
		@ism:nonICmarkings is not specified with a value containing more than one of the tokens [XD],
		[ND], [SBU], or [SBU-NF]. 
	</sch:p>
	<sch:param name="context" value="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('XD', 'ND', 'SBU', 'SBU-NF'))]"/>
	<sch:param name="attrValue" value="@ism:nonICmarkings"/>
	<sch:param name="mutuallyExclusiveTokenList" value="('XD', 'ND', 'SBU', 'SBU-NF')"/>
	<sch:param name="errMsg" value="'   [ISM-ID-00038][Error] If ISM_USGOV_RESOURCE, then the tokens    [XD], [ND], [SBU], and [SBU-NF] are mutually exclusive for attribute nonICmarkings.      Human Readable: USA documents must not specify [XD], [ND], [SBU], and/or [SBU-NF] commingled on a single element.   '"/>
</sch:pattern>