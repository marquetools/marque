<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="NoticeHasCorrespondingDataTwoPossibleTokens">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		Abstract pattern to ensure that for a given element in a document that is
		ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, either $dataType or $dataType2 exists in $dataTokenList. The calling rule must pass $dataType1,
		$dataType2, $noticeType, $attrName and $dataTokenList.  This rule was created because the token for Law Enforcement data
	    is [LES] in @ism:disseminationControls and [LEI] in @ism:cuiBasic, but both tokens require an [LES] notice.
	</sch:p>
	<sch:rule id="NoticeHasCorrespondingDataTwoPossibleTokens-R1" context="*[($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE) and util:contributesToRollup(.) and not(@ism:externalNotice = true()) and util:containsAnyOfTheTokens(@ism:noticeType, ($noticeType))]">
		<sch:assert test="(index-of($dataTokenList, $dataType1) &gt; 0)
			or (index-of($dataTokenList, $dataType2) &gt; 0)" flag="error" role="error">
			[<sch:value-of select="$ruleId"/>][Error] If ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, and any element
			meeting ISM_CONTRIBUTES in the document has the attribute noticeType containing
				[<sch:value-of select="$noticeType"/>], then some element meeting ISM_CONTRIBUTES in
			the document MUST have attribute <sch:value-of select="$attrName"/> containing
			[<sch:value-of select="$dataType1"/>] or [<sch:value-of select="$dataType2"/>], respectively. Human Readable: USA documents containing an
				<sch:value-of select="$noticeType"/> notice must also have either [<sch:value-of
					select="$dataType1"/>] or [<sch:value-of select="$dataType2"/>] data. </sch:assert>
	</sch:rule>
</sch:pattern>
