<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="DataHasCorrespondingNotice">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		Abstract pattern to enforce that an appropriate notice exists for an
		element in $partTags that has a notice requirement. The calling rule must pass $elem,
		$attrName, $partTags, and $noticeType.
	</sch:p>
	<sch:rule id="DataHasCorrespondingNotice-R1" context="*[($ISM_USGOV_RESOURCE or $ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE)
		and util:contributesToRollup(.) and util:containsAnyOfTheTokens($attrValue, ($dataType))]">
		<sch:assert test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ($noticeType)) and not ($elem/@ism:externalNotice=true()))" flag="error" role="error">
			[<sch:value-of select="$ruleId"/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <sch:value-of select="$attrName"/> containing [<sch:value-of select="$dataType"/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<sch:value-of select="$noticeType"/>].</sch:assert>
	</sch:rule>
</sch:pattern>