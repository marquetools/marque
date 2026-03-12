<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="NoticeHasCorrespondingDataUnclassDoc">
	<sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		Abstract pattern to ensure that for a given element in an
		ISM_USGOV_RESOURCE with @ism:noticeType containing a specified token and @ism:externalNotice
		not equal true, $dataType exists in $dataTokenList ONLY if the $ISM_RESOURCE_ELEMENT is Unclassified.
		The calling rule must pass $dataType and @dataTokenList.
	</sch:p>
	<sch:rule id="NoticeHasCorrespondingDataUnclassDoc-R1" context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:classification, ('U'))   and not (@ism:externalNotice=true()) and util:containsAnyOfTheTokens(@ism:noticeType, ($dataType))]">
		<sch:assert test="index-of($dataTokenList, $dataType)&gt;0" flag="error" role="error"> [<sch:value-of select="$ruleId"/>][Error] If ISM_USGOV_RESOURCE and any element meeting
			ISM_CONTRIBUTES in the document has the attribute noticeType containing [<sch:value-of select="$dataType"/>], then some element meeting ISM_CONTRIBUTES in the document
			MUST have attribute <sch:value-of select="$attrName"/> containing [<sch:value-of select="$dataType"/>]. Human Readable: USA documents containing an <sch:value-of select="$dataType"/> notice must also have <sch:value-of select="$dataType"/> data.
		</sch:assert>
	</sch:rule>
</sch:pattern>