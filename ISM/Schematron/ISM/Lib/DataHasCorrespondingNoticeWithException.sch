<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="DataHasCorrespondingNoticeWithException">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		For all elements that contribute to rollup when all of the following are true:
		(a) the given expression $attrValue contains the given value $noticeType
		(b) the given exception expression $exceptAttrValue does not contain the given exception value $exceptNoticeType
		(c) $ISM_USGOV_RESOURCE is true
		
		Assert that some non-resource node element satisfies both
		(a) @ism:noticeType contains the $noticeType token
		(b) not(@ism:externalNotice is true)

		This rule depends on $partTags defined in the ISM_XML.sch master Schematron file.
		
		The calling rule must pass $attrValue, $noticeType, $exceptAttrValue, $exceptNoticeType.
	</sch:p>

	<sch:rule id="DataHasCorrespondingNoticeWithException-R1" context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and util:containsAnyOfTheTokens($attrValue, ($noticeType)) and not(util:containsAnyOfTheTokens($exceptAttrValue, ($exceptNoticeType)))]">
		<sch:assert test="some $elem in $partTags satisfies ($elem[@ism:noticeType] and util:containsAnyOfTheTokens($elem/@ism:noticeType, ($noticeType)) and not($elem/@ism:externalNotice = true()))" flag="error" role="error">
			[<sch:value-of select="$ruleId"/>][Error] If ISM_USGOV_RESOURCE, any
			element meeting ISM_CONTRIBUTES in the document has the attribute <sch:value-of select="$attrName"/> containing [<sch:value-of select="$noticeType"/>], then some
			element meeting ISM_CONTRIBUTES in the document MUST have attribute noticeType
			containing [<sch:value-of select="$noticeType"/>].</sch:assert>
	</sch:rule>
</sch:pattern>