<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00358">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
	  	[ISM-ID-00358][Error] A document using tetragraphs may not have a releasableTo that is less restrictive 
	  	than that of any tetragraph or organization tokens used in the releasableTo fields.
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	Determine the set of releasable countries by determining, for each token, if it is a country code or tetragraph.
	  	If it is a tetragraph, get the membership from ISMCAT Taxonomy, otherwise add the token to the list. Then determine if any
	  	of the tetragraph tokens have releasability restrictions themselves. If so, add that token to a list. Finally,
	  	determine if the releasability of the tetragraph tokens are more restrictive then the releasability of the document.
	  	If there are, trigger the error message.
	</sch:p>
	  <sch:rule id="ISM-ID-00358-R1" context="*[@ism:resourceElement=true()][1]">
	  	<sch:let name="op" value="if(@ism:joint=true()) then @ism:ownerProducer else ''"/>
	  	<sch:let name="releasableToCountries" value="distinct-values(for $value in tokenize(normalize-space(concat(@ism:releasableTo,' ',$op)),' ') return      if(index-of($catt//catt:TetraToken,$value)&gt;0)      then util:tokenize(util:getTetragraphMembership($value))      else $value)"/>	  	
	  	<sch:let name="tetrasWithReleasableTo" value="distinct-values(for $value in $tetras return if($catt//catt:TetraToken[.=$value]/@ism:releasableTo)         then $value        else null)"/>	  	
		<sch:let name="moreRestrictiveTetras" value="for $tetra in $tetrasWithReleasableTo return       if (every $value in $releasableToCountries satisfies index-of(distinct-values(util:tokenize(util:getTetragraphReleasability($tetra))),$value)&gt;0)        then null else $tetra"/>
		<sch:assert test="empty($moreRestrictiveTetras)" flag="error" role="error">
			[ISM-ID-00358][Error] A document using tetragraphs may not have a releasableTo that is less restrictive 
			than that of any tetragraph or organization tokens used in the releasableTo fields. The following tetragraphs
			have a more restrictive releasability than the document: <sch:value-of select="string-join($moreRestrictiveTetras,', ')"/>
		</sch:assert>
	  	<sch:assert test="exists($catt//catt:Tetragraphs)" flag="error" role="error">ISMCAT Taxonomy does not exist!</sch:assert>
	  </sch:rule>
</sch:pattern>